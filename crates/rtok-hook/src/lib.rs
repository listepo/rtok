//! T178 / D32: the wire between `rtok-hook`, a std-only client a host starts once per hook call,
//! and `rtok hook --serve`, the optional resident that runs the hook without a process start.
//!
//! A frame is a little-endian `u32` length, then the body. Request body: length-prefixed fields
//! `version, fingerprint, event, host, cwd, stdin`. Response body: status byte `0` and the hook's
//! stdout, or `1` alone — refused, and the client runs `rtok hook` itself.

use std::ffi::OsString;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// Largest body either end accepts: a PostToolUse payload carries the whole tool output.
pub const MAX_FRAME: usize = 64 << 20;

/// Variables besides `RTOK_*` that change the hook's config.
const CONFIG_VARS: &[&str] = &[
    "HOME",
    "USERPROFILE",
    "OTEL_EXPORTER_OTLP_ENDPOINT",
    "OTEL_EXPORTER_OTLP_HEADERS",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// The client's rtok version; the resident serves only its own.
    pub version: String,
    /// [`fingerprint`] of the client's environment; the resident serves only its own.
    pub fingerprint: u64,
    pub event: String,
    /// `--host`, empty when not given.
    pub host: String,
    /// The client's working directory; the hook runs there.
    pub cwd: String,
    pub stdin: Vec<u8>,
}

impl Request {
    /// The whole frame, length included.
    pub fn encode(&self) -> Vec<u8> {
        let fp = self.fingerprint.to_le_bytes();
        let fields: [&[u8]; 6] = [
            self.version.as_bytes(),
            &fp,
            self.event.as_bytes(),
            self.host.as_bytes(),
            self.cwd.as_bytes(),
            &self.stdin,
        ];
        let mut body = Vec::new();
        for f in fields {
            body.extend_from_slice(&(f.len() as u32).to_le_bytes());
            body.extend_from_slice(f);
        }
        frame(body)
    }

    /// A body as [`read_frame`] returns it; `None` when malformed.
    pub fn decode(mut body: &[u8]) -> Option<Self> {
        let text = |b: &[u8]| String::from_utf8(b.to_vec()).ok();
        let req = Self {
            version: text(take(&mut body)?)?,
            fingerprint: u64::from_le_bytes(take(&mut body)?.try_into().ok()?),
            event: text(take(&mut body)?)?,
            host: text(take(&mut body)?)?,
            cwd: text(take(&mut body)?)?,
            stdin: take(&mut body)?.to_vec(),
        };
        body.is_empty().then_some(req)
    }
}

/// The response frame: `Some(stdout)` served, `None` refused.
pub fn encode_response(stdout: Option<&[u8]>) -> Vec<u8> {
    let mut body = vec![u8::from(stdout.is_none())];
    body.extend_from_slice(stdout.unwrap_or_default());
    frame(body)
}

/// A response body: `Some(Some(stdout))` served, `Some(None)` refused, `None` malformed.
pub fn decode_response(body: &[u8]) -> Option<Option<Vec<u8>>> {
    match body.split_first()? {
        (0, out) => Some(Some(out.to_vec())),
        (1, []) => Some(None),
        _ => None,
    }
}

/// One frame's body.
pub fn read_frame(r: &mut impl Read) -> io::Result<Vec<u8>> {
    let mut header = [0; 4];
    r.read_exact(&mut header)?;
    let mut body = vec![0; frame_len(header)?];
    r.read_exact(&mut body)?;
    Ok(body)
}

/// The body length a frame header announces, refusing anything over [`MAX_FRAME`].
pub fn frame_len(header: [u8; 4]) -> io::Result<usize> {
    let len = u32::from_le_bytes(header) as usize;
    if len > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame too large",
        ));
    }
    Ok(len)
}

/// FNV-1a over the `RTOK_*` and [`CONFIG_VARS`] pairs of `vars`, in any order; every other
/// variable is ignored. Equal fingerprints mean `rtok hook` would load the same config.
pub fn fingerprint(vars: impl IntoIterator<Item = (OsString, OsString)>) -> u64 {
    let mut kept: Vec<_> = vars
        .into_iter()
        .filter(|(k, _)| {
            let k = k.to_string_lossy();
            k.starts_with("RTOK_") || CONFIG_VARS.contains(&k.as_ref())
        })
        .collect();
    kept.sort();
    let mut bytes = Vec::new();
    for (k, v) in kept {
        bytes.extend_from_slice(k.as_encoded_bytes());
        bytes.push(0);
        bytes.extend_from_slice(v.as_encoded_bytes());
        bytes.push(0);
    }
    fnv(&bytes)
}

/// The resident's home from raw variables: `$RTOK_HOME` when absolute, else
/// `<HOME or USERPROFILE>/.rtok`. `None` (no resident) when `RTOK_HOME` needs `~` expansion.
pub fn home(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let set = |k: &str| var(k).filter(|v| !v.is_empty());
    match set("RTOK_HOME") {
        Some(h) => Some(PathBuf::from(h)).filter(|p| p.is_absolute()),
        None => Some(PathBuf::from(set("HOME").or_else(|| set("USERPROFILE"))?).join(".rtok")),
    }
}

/// Unix: `<home>/hook.sock`, `None` past the 104-byte `sun_path`. Windows: a pipe named by home.
pub fn endpoint(home: &Path) -> Option<PathBuf> {
    if cfg!(windows) {
        let id = fnv(home.as_os_str().as_encoded_bytes());
        return Some(PathBuf::from(format!(r"\\.\pipe\rtok-hook-{id:016x}")));
    }
    let path = home.join("hook.sock");
    (path.as_os_str().len() < 100).then_some(path)
}

fn frame(body: Vec<u8>) -> Vec<u8> {
    let mut out = (body.len() as u32).to_le_bytes().to_vec();
    out.extend(body);
    out
}

fn take<'a>(body: &mut &'a [u8]) -> Option<&'a [u8]> {
    let (len, rest) = body.split_first_chunk::<4>()?;
    let len = u32::from_le_bytes(*len) as usize;
    let field = rest.get(..len)?;
    *body = &rest[len..];
    Some(field)
}

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
        pairs.iter().map(|(k, v)| (k.into(), v.into())).collect()
    }

    #[test]
    fn a_truncated_request_is_refused_and_only_config_variables_move_the_fingerprint() {
        let req = Request {
            version: "0.7.0".into(),
            fingerprint: 42,
            event: "PreToolUse".into(),
            host: String::new(),
            cwd: "/w".into(),
            stdin: b"{}".to_vec(),
        };
        let body = read_frame(&mut &req.encode()[..]).unwrap();
        assert_eq!(Request::decode(&body[..body.len() - 1]), None);
        let base = fingerprint(vars(&[("RTOK_HOME", "/h"), ("HOME", "/u")]));
        assert_eq!(
            base,
            fingerprint(vars(&[("HOME", "/u"), ("PWD", "/x"), ("RTOK_HOME", "/h")]))
        );
        assert_ne!(
            base,
            fingerprint(vars(&[("RTOK_HOME", "/h"), ("HOME", "/v")]))
        );
    }

    #[test]
    fn home_needs_an_absolute_rtok_home_or_a_user_home() {
        let home_of = |pairs: &[(&str, &str)]| {
            let env = vars(pairs);
            home(|k| env.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone()))
        };
        let abs = if cfg!(windows) { r"C:\h" } else { "/h" };
        assert_eq!(home_of(&[("RTOK_HOME", "~/x"), ("HOME", "/u")]), None);
        assert_eq!(home_of(&[("RTOK_HOME", abs)]), Some(abs.into()));
        assert_eq!(
            home_of(&[("HOME", "/u")]),
            Some(Path::new("/u").join(".rtok"))
        );
    }
}
