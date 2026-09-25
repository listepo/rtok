//! T178 / D32: rtok's home resolver ([`home`]), and the wire between `rtok-hook`, a std-only
//! client a host starts once per hook call, and `rtok hook --serve`, the optional resident that
//! runs the hook without a process start.
//!
//! A frame is a little-endian `u32` length, then the body. Request body: length-prefixed fields
//! `version, fingerprint, event, host, cwd, stdin`. Response body: status byte `0` and the hook's
//! stdout, or `1` alone — refused, and the client runs `rtok hook` itself.

use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// Largest body either end accepts: a PostToolUse payload carries the whole tool output.
pub const MAX_FRAME: usize = 64 << 20;

/// Variables the hook reads per call instead of from its config. A client that has one runs
/// `rtok hook` itself, and they are part of [`fingerprint`] so a resident started with one serves
/// no call without it.
pub const HOST_VARS: &[&str] = &["GROK_HOOK_EVENT", "DEVIN_PROJECT_DIR"];

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

/// A connection to the resident at `endpoint`: a Unix socket, or a named pipe on Windows. `None`
/// when nothing listens there.
pub fn connect(endpoint: &Path) -> Option<impl Read + Write + Send + 'static> {
    #[cfg(unix)]
    return std::os::unix::net::UnixStream::connect(endpoint).ok();
    #[cfg(windows)]
    return std::fs::File::options()
        .read(true)
        .write(true)
        .open(endpoint)
        .ok();
}

/// Send one request frame and read the answer, as [`decode_response`] reads it; `None` on any
/// I/O error or a malformed answer.
pub fn exchange(s: &mut (impl Read + Write), frame: &[u8]) -> Option<Option<Vec<u8>>> {
    s.write_all(frame).ok()?;
    decode_response(&read_frame(s).ok()?)
}

/// FNV-1a over the `RTOK_*`, [`CONFIG_VARS`] and [`HOST_VARS`] pairs of `vars`, in any order;
/// every other variable is ignored. Equal fingerprints mean `rtok hook` reads the same inputs.
pub fn fingerprint(vars: impl IntoIterator<Item = (OsString, OsString)>) -> u64 {
    let mut kept: Vec<_> = vars
        .into_iter()
        .filter(|(k, _)| {
            let k = k.to_string_lossy();
            k.starts_with("RTOK_") || CONFIG_VARS.iter().chain(HOST_VARS).any(|v| *v == k)
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

/// rtok's home from raw variables, resolved exactly as `rtok` resolves it ([`home_dir_from`]),
/// so client and resident agree on the endpoint. `None` when that is not absolute: no resident
/// hangs off the cwd.
pub fn home(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let user_home = user_home_from(var("HOME"), var("USERPROFILE"));
    Some(home_dir_from(var("RTOK_HOME"), user_home)).filter(|p| p.is_absolute())
}

/// The user home: `HOME`, else `USERPROFILE`. Native Windows PowerShell often has `HOME` unset or
/// empty, and an empty one must not block the fallback.
pub fn user_home_from(home: Option<OsString>, userprofile: Option<OsString>) -> Option<PathBuf> {
    let nonempty = |v: Option<OsString>| v.filter(|v| !v.is_empty()).map(PathBuf::from);
    nonempty(home).or_else(|| nonempty(userprofile))
}

/// `$RTOK_HOME`, else `<user home>/.rtok`. A `~` in `RTOK_HOME` (set from a JSON `env` block,
/// where no shell expands it) is expanded: left literal, every store path hung off it resolved
/// against the cwd as `./~/.rtok/…` (T169).
///
/// May return a path relative to the (unknown, to this function) cwd when `rtok_home` is itself
/// relative, or when neither it nor `user_home` is given at all — [`home`] relies on exactly
/// this to know when a resident would depend on the caller's cwd. `Config::home_dir` (T184) is
/// the one that must never hand back a relative path; it post-processes this result instead of
/// changing what this function returns.
pub fn home_dir_from(rtok_home: Option<OsString>, user_home: Option<PathBuf>) -> PathBuf {
    let default = user_home.clone().unwrap_or_default().join(".rtok");
    match rtok_home {
        Some(h) => expand_with(Path::new(&h), &default, user_home.as_deref()),
        None => default,
    }
}

/// `~/.rtok/x` → `<rtok_home>/x` (so `RTOK_HOME` moves the whole tree), other `~/x` →
/// `<user_home>/x`. Bare `~` and `~/.rtok` (no trailing slash) expand too — leaving them
/// literal is how tests without `finish` used to create a `./~` directory in the repo. The
/// user home is explicit so the Windows `USERPROFILE` fallback is testable without env.
pub fn expand_with(path: &Path, rtok_home: &Path, user_home: Option<&Path>) -> PathBuf {
    let raw = path.to_string_lossy();
    if let Some(rest) = strip_rtok_home_prefix(&raw) {
        return match rest {
            "" => rtok_home.to_path_buf(),
            rest => join_tilde_rest(rtok_home, rest),
        };
    }
    if raw == "~" {
        return user_home
            .map(Path::to_path_buf)
            .unwrap_or_else(|| path.to_path_buf());
    }
    if let Some(rest) = strip_home_prefix(&raw) {
        return match user_home {
            Some(h) => join_tilde_rest(h, rest),
            None => path.to_path_buf(),
        };
    }
    path.to_path_buf()
}

fn strip_rtok_home_prefix(raw: &str) -> Option<&str> {
    for prefix in ["~/.rtok/", "~/.rtok\\", "~\\.rtok\\", "~\\.rtok/"] {
        if let Some(rest) = raw.strip_prefix(prefix) {
            return Some(rest);
        }
    }
    match raw {
        "~/.rtok" | "~/.rtok/" | "~/.rtok\\" | "~\\.rtok" | "~\\.rtok\\" | "~\\.rtok/" => Some(""),
        _ => None,
    }
}

fn strip_home_prefix(raw: &str) -> Option<&str> {
    raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\"))
}

/// Join a tilde-relative remainder that may use `/` or `\\` separators.
fn join_tilde_rest(base: &Path, rest: &str) -> PathBuf {
    let mut out = base.to_path_buf();
    for part in rest.split(['/', '\\']).filter(|s| !s.is_empty()) {
        out.push(part);
    }
    out
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
    fn home_is_rtoks_own_and_never_relative() {
        let home_of = |pairs: &[(&str, &str)]| {
            let env = vars(pairs);
            home(|k| env.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone()))
        };
        let (abs, user) = if cfg!(windows) {
            (r"C:\h", r"C:\u")
        } else {
            ("/h", "/u")
        };
        let user_home = Path::new(user);
        assert_eq!(home_of(&[("RTOK_HOME", abs)]), Some(abs.into()));
        // T169: a literal `~` expands, as `rtok` expands it.
        let tilde = home_of(&[("RTOK_HOME", "~/x"), ("HOME", user)]);
        assert_eq!(tilde, Some(user_home.join("x")));
        let rtok_tilde = home_of(&[("RTOK_HOME", "~/.rtok"), ("HOME", user)]);
        assert_eq!(rtok_tilde, Some(user_home.join(".rtok")));
        let profile = home_of(&[("HOME", ""), ("USERPROFILE", user)]);
        assert_eq!(profile, Some(user_home.join(".rtok")));
        // A home relative to the cwd (T184) serves no resident.
        assert_eq!(home_of(&[("RTOK_HOME", "rel")]), None);
        assert_eq!(home_of(&[]), None);
    }
}
