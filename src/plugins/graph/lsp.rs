//! Optional LSP backend (T30.2). Native JSON-RPC over stdio.
//! Spawns rust-analyzer / clangd / typescript-language-server from PATH (D6: not serena).

use std::collections::{BTreeMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use rtok_plugin_sdk::{Class, Ctx, Measurement};
use serde_json::{Value, json};

use super::cap;

const READY: Duration = Duration::from_secs(40);

pub(crate) fn on_path(bin: &str) -> bool {
    Command::new(bin)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn resolve_bin(name: &str) -> PathBuf {
    if name == "rust-analyzer"
        && let Ok(o) = Command::new("rustup")
            .args(["which", "rust-analyzer"])
            .stderr(Stdio::null())
            .output()
        && o.status.success()
    {
        let p = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if !p.is_empty() && Path::new(&p).is_file() {
            return PathBuf::from(p);
        }
    }
    PathBuf::from(name)
}

fn pick(root: &Path) -> Result<(&'static str, &'static [&'static str])> {
    if root.join("Cargo.toml").is_file() {
        return Ok(("rust-analyzer", &[]));
    }
    if root.join("compile_commands.json").is_file() {
        return Ok(("clangd", &[]));
    }
    if root.join("tsconfig.json").is_file() {
        return Ok(("typescript-language-server", &["--stdio"]));
    }
    if root.join("pubspec.yaml").is_file() {
        return Ok(("dart", &["language-server"]));
    }
    bail!(
        "lsp: no Cargo.toml / compile_commands.json / tsconfig.json / pubspec.yaml in {}",
        root.display()
    )
}

fn percent_encode_path(s: &str) -> String {
    // RFC 8089 / URI path: encode spaces and other non-unreserved octets; keep `/` and
    // Windows drive `C:` intact so rust-analyzer still accepts the URI.
    let mut out = String::with_capacity(s.len());
    for (i, seg) in s.split('/').enumerate() {
        if i > 0 {
            out.push('/');
        }
        if seg.len() == 2 && seg.as_bytes().get(1) == Some(&b':') {
            out.push_str(seg);
            continue;
        }
        for b in seg.bytes() {
            match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                    out.push(b as char)
                }
                _ => out.push_str(&format!("%{b:02X}")),
            }
        }
    }
    out
}

fn percent_decode_path(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let h = (bytes[i + 1] as char).to_digit(16);
            let l = (bytes[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (h, l) {
                out.push(((h << 4) | l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn file_uri(p: &Path) -> String {
    // LSP wants RFC 8089 `file:///C:/…` on Windows. `.display()` keeps
    // backslashes and omits the third slash, which rust-analyzer rejects.
    let path = dunce::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let s = percent_encode_path(&path.to_string_lossy().replace('\\', "/"));
    if s.starts_with('/') {
        format!("file://{s}")
    } else {
        format!("file:///{s}")
    }
}

fn path_from_file_uri(uri: &str) -> PathBuf {
    let rest = percent_decode_path(uri.strip_prefix("file://").unwrap_or(uri));
    // `file:///C:/Users/…` → `C:/Users/…`; `file:///home/…` keeps the root slash.
    if cfg!(windows) {
        let trimmed = rest.trim_start_matches('/');
        if trimmed.len() >= 2 && trimmed.as_bytes()[1] == b':' {
            return PathBuf::from(trimmed);
        }
    }
    PathBuf::from(rest)
}

fn rel(root: &Path, uri: &str) -> String {
    let p = path_from_file_uri(uri);
    pathdiff::diff_paths(&p, root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| p.display().to_string())
}

fn kind_name(k: u64) -> &'static str {
    match k {
        5 => "class",
        6 => "method",
        11 => "enum",
        12 => "function",
        13 => "variable",
        23 => "struct",
        _ => "symbol",
    }
}

fn write_msg(w: &mut impl Write, v: &Value) -> Result<()> {
    let body = serde_json::to_vec(v)?;
    write!(w, "Content-Length: {}\r\n\r\n", body.len())?;
    w.write_all(&body)?;
    w.flush()?;
    Ok(())
}

fn read_msg(r: &mut impl BufRead) -> Result<Value> {
    let mut len = None;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line)? == 0 {
            bail!("lsp: eof");
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        if let Some(rest) = line
            .split_once(':')
            .filter(|(k, _)| k.eq_ignore_ascii_case("content-length"))
            .map(|(_, v)| v)
        {
            len = Some(rest.trim().parse::<usize>().context("Content-Length")?);
        }
    }
    let n = len.context("lsp: no Content-Length")?;
    let mut buf = vec![0; n];
    r.read_exact(&mut buf)?;
    serde_json::from_slice(&buf).context("lsp json")
}

struct Session {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: i64,
    root: PathBuf,
    opened: HashSet<String>,
    err_path: PathBuf,
}

impl Session {
    fn spawn(root: &Path) -> Result<Self> {
        let (bin, args) = pick(root)?;
        if !on_path(bin) {
            bail!("lsp: {bin} not on PATH");
        }
        let path = resolve_bin(bin);
        let err_path = std::env::temp_dir().join(format!("rtok-lsp-{}.stderr", std::process::id()));
        let mut cmd = Command::new(&path);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .current_dir(root);
        match std::fs::File::create(&err_path) {
            Ok(f) => {
                cmd.stderr(Stdio::from(f));
            }
            Err(_) => {
                cmd.stderr(Stdio::null());
            }
        }
        let mut child = cmd
            .spawn()
            .with_context(|| format!("spawn {} ({bin})", path.display()))?;
        let stdin = child.stdin.take().context("lsp stdin")?;
        let stdout = BufReader::new(child.stdout.take().context("lsp stdout")?);
        let mut s = Self {
            child,
            stdin,
            stdout,
            next_id: 1,
            root: dunce::canonicalize(root).unwrap_or_else(|_| root.to_path_buf()),
            opened: HashSet::new(),
            err_path,
        };
        let uri = file_uri(&s.root);
        if let Err(e) = s.request(
            "initialize",
            json!({
                // Ours, not null: the server watches it and exits when rtok dies, even by
                // SIGKILL, instead of outliving the `mcp` that spawned it.
                "processId": std::process::id(),
                "rootUri": uri,
                "rootPath": s.root,
                "capabilities": {
                    "workspace": {
                        "configuration": true,
                        "workspaceFolders": true
                    },
                    "textDocument": {
                        "documentSymbol": {"hierarchicalDocumentSymbolSupport": true}
                    },
                    "window": {"workDoneProgress": true}
                },
                "workspaceFolders": [{"uri": uri, "name": "root"}]
            }),
        ) {
            return Err(s.dead(e));
        }
        s.notify("initialized", json!({}))?;
        Ok(s)
    }

    fn dead(&mut self, e: anyhow::Error) -> anyhow::Error {
        let st = self.child.try_wait().ok().flatten();
        let err = std::fs::read_to_string(&self.err_path).unwrap_or_default();
        let err = err.trim();
        match (st, err.is_empty()) {
            (Some(st), false) => anyhow::anyhow!("{e}; exited {st}; stderr: {err}"),
            (Some(st), true) => anyhow::anyhow!("{e}; exited {st}"),
            (None, false) => anyhow::anyhow!("{e}; stderr: {err}"),
            (None, true) => e,
        }
    }

    fn notify(&mut self, method: &str, params: Value) -> Result<()> {
        write_msg(
            &mut self.stdin,
            &json!({"jsonrpc":"2.0","method": method, "params": params}),
        )
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = json!(self.next_id);
        self.next_id += 1;
        write_msg(
            &mut self.stdin,
            &json!({"jsonrpc":"2.0","id": id, "method": method, "params": params}),
        )?;
        let deadline = Instant::now() + READY;
        loop {
            if Instant::now() > deadline {
                bail!("lsp: timeout waiting for {method}");
            }
            let msg = match read_msg(&mut self.stdout) {
                Ok(m) => m,
                Err(e) => return Err(self.dead(e)),
            };
            if msg.get("method").is_some() && msg.get("id").is_some_and(|i| !i.is_null()) {
                self.reply_server(&msg)?;
                continue;
            }
            if msg.get("id") == Some(&id) {
                if let Some(err) = msg.get("error") {
                    bail!("lsp {method}: {err}");
                }
                return Ok(msg["result"].clone());
            }
        }
    }

    fn reply_server(&mut self, msg: &Value) -> Result<()> {
        let id = &msg["id"];
        let result = match msg["method"].as_str().unwrap_or("") {
            "workspace/configuration" => {
                let n = msg["params"]["items"]
                    .as_array()
                    .map(|a| a.len())
                    .unwrap_or(1);
                json!(vec![json!({}); n])
            }
            "workspace/workspaceFolders" => {
                json!([{"uri": file_uri(&self.root), "name": "root"}])
            }
            _ => json!(null),
        };
        write_msg(
            &mut self.stdin,
            &json!({"jsonrpc":"2.0","id": id, "result": result}),
        )
    }

    fn did_open(&mut self, uri: &str) -> Result<()> {
        if !self.opened.insert(uri.to_string()) {
            return Ok(());
        }
        let path = path_from_file_uri(uri);
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let language_id = match path.extension().and_then(|e| e.to_str()) {
            Some("rs") => "rust",
            Some("dart") => "dart",
            _ => "plaintext",
        };
        self.notify(
            "textDocument/didOpen",
            json!({"textDocument": {
                "uri": uri, "languageId": language_id, "version": 1, "text": text
            }}),
        )
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = write_msg(&mut self.stdin, &json!({"jsonrpc":"2.0","method":"exit"}));
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.err_path);
    }
}

/// The one cached server, keyed by workspace root. A `static` is never dropped, so
/// [`shutdown`] is what runs `Session::drop` when `rtok mcp` ends.
static SESSION: Mutex<Option<(String, Session)>> = Mutex::new(None);

fn with_session<T>(root: &Path, f: impl FnOnce(&mut Session) -> Result<T>) -> Result<T> {
    let key = super::index::canon(root);
    let mut g = SESSION.lock().unwrap_or_else(|e| e.into_inner());
    let restart = match g.as_mut() {
        Some((k, s)) if *k == key => s.child.try_wait().ok().flatten().is_some(),
        _ => true,
    };
    if restart {
        // Reap the old server first: both share one stderr path, and dropping it after the
        // spawn deleted the file its successor had just opened.
        *g = None;
        *g = Some((key, Session::spawn(root)?));
    }
    f(&mut g.as_mut().expect("session").1)
}

/// Stop the cached language server (`exit`, kill, reap) and remove its stderr file.
pub(crate) fn shutdown() {
    drop(SESSION.lock().unwrap_or_else(|e| e.into_inner()).take());
}

struct Def {
    uri: String,
    pos: Value,
    kind: u64,
    path: String,
    line: i32,
    end_line: i32,
}

fn pick_def(r: &Value, name: &str, root: &Path) -> Option<Def> {
    r.as_array()?.iter().find_map(|it| {
        if it["name"].as_str()? != name {
            return None;
        }
        let loc = &it["location"];
        let uri = loc["uri"].as_str()?.to_string();
        let start = loc["range"]["start"].clone();
        let end_line = loc["range"]["end"]["line"].as_i64().unwrap_or(0) as i32 + 1;
        Some(Def {
            line: start["line"].as_i64().unwrap_or(0) as i32 + 1,
            pos: start,
            kind: it["kind"].as_u64().unwrap_or(0),
            path: rel(root, loc["uri"].as_str()?),
            end_line,
            uri,
        })
    })
}

fn wait_def(s: &mut Session, name: &str) -> Result<Option<Def>> {
    let deadline = Instant::now() + READY;
    loop {
        let r = s.request("workspace/symbol", json!({"query": name}))?;
        if let Some(d) = pick_def(&r, name, &s.root) {
            return Ok(Some(d));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

fn contains(range: &Value, line: u64, ch: u64) -> bool {
    let sl = range["start"]["line"].as_u64().unwrap_or(0);
    let sc = range["start"]["character"].as_u64().unwrap_or(0);
    let el = range["end"]["line"].as_u64().unwrap_or(0);
    let ec = range["end"]["character"].as_u64().unwrap_or(0);
    (line > sl || (line == sl && ch >= sc)) && (line < el || (line == el && ch <= ec))
}

fn enclosing(syms: &[Value], line: u64, ch: u64) -> String {
    let mut best = String::new();
    fn walk(syms: &[Value], line: u64, ch: u64, best: &mut String) {
        for s in syms {
            if contains(&s["range"], line, ch) {
                if let Some(n) = s["name"].as_str() {
                    *best = n.to_string();
                }
                if let Some(c) = s["children"].as_array() {
                    walk(c, line, ch, best);
                }
            }
        }
    }
    walk(syms, line, ch, &mut best);
    best
}

fn finish(cx: &Ctx, tool: &'static str, t0: Instant, text: String) -> Result<String> {
    let kind = match tool {
        "symbol" => "lsp.symbol",
        "callers" => "lsp.callers",
        "outline" => "lsp.outline",
        _ => "lsp.impact",
    };
    let est = cx.estimate(&text, Class::Code);
    cx.record(&Measurement {
        plugin: "graph",
        kind,
        before_bytes: t0.elapsed().as_millis() as u64,
        after_bytes: text.len() as u64,
        est_before: est,
        est_after: est,
        ref_id: None,
        call_id: cx.call_id(),
    })?;
    cap(cx, text)
}

fn body(root: &Path, path: &str, line: i32, end_line: i32, budget: usize) -> String {
    let src = std::fs::read_to_string(root.join(path)).unwrap_or_default();
    super::body_lines(&src, line, end_line, budget)
}

pub(crate) fn symbol(cx: &Ctx, root: &Path, name: &str, filter: &super::Filter) -> Result<String> {
    let t0 = Instant::now();
    let budget = cx.plugin_config::<crate::config::Graph>("graph").body_lines as usize;
    with_session(root, |s| {
        if wait_def(s, name)?.is_none() {
            return finish(
                cx,
                "symbol",
                t0,
                format!("no definition of {name}{}", filter.scope_note()),
            );
        }
        let r = s.request("workspace/symbol", json!({"query": name}))?;
        let mut out = String::new();
        if let Some(arr) = r.as_array() {
            for it in arr {
                if it["name"].as_str() != Some(name) {
                    continue;
                }
                let Some(d) = pick_def(&json!([it]), name, &s.root) else {
                    continue;
                };
                if !filter.path_ok(&d.path) || !filter.kind_ok(kind_name(d.kind)) {
                    continue;
                }
                out.push_str(&format!("{}:{} {}\n", d.path, d.line, kind_name(d.kind)));
                out.push_str(&body(&s.root, &d.path, d.line, d.end_line, budget));
            }
        }
        if out.is_empty() {
            out = format!("no definition of {name}{}", filter.scope_note());
        }
        finish(cx, "symbol", t0, out)
    })
}

pub(crate) fn callers(cx: &Ctx, root: &Path, name: &str, filter: &super::Filter) -> Result<String> {
    let t0 = Instant::now();
    with_session(root, |s| {
        let Some(d) = wait_def(s, name)? else {
            return finish(
                cx,
                "callers",
                t0,
                format!("no references to {name}{}", filter.scope_note()),
            );
        };
        s.did_open(&d.uri)?;
        let refs = s.request(
            "textDocument/references",
            json!({
                "textDocument": {"uri": d.uri},
                "position": d.pos,
                "context": {"includeDeclaration": false}
            }),
        )?;
        let mut groups: BTreeMap<(String, String), (i64, i32)> = BTreeMap::new();
        let mut syms_by: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        for loc in refs.as_array().cloned().unwrap_or_default() {
            let uri = loc["uri"].as_str().unwrap_or("");
            s.did_open(uri)?;
            let path = rel(&s.root, uri);
            let line0 = loc["range"]["start"]["line"].as_u64().unwrap_or(0);
            let ch = loc["range"]["start"]["character"].as_u64().unwrap_or(0);
            let line = line0 as i32 + 1;
            let kids = if let Some(existing) = syms_by.get(uri) {
                existing.clone()
            } else {
                let raw = s.request(
                    "textDocument/documentSymbol",
                    json!({"textDocument": {"uri": uri}}),
                )?;
                let kids = raw.as_array().cloned().unwrap_or_default();
                syms_by.insert(uri.to_string(), kids.clone());
                kids
            };
            let scope = enclosing(&kids, line0, ch);
            let e = groups.entry((path, scope)).or_insert((0, line));
            e.0 += 1;
            if line < e.1 {
                e.1 = line;
            }
        }
        if groups.is_empty() {
            return finish(
                cx,
                "callers",
                t0,
                format!("no references to {name}{}", filter.scope_note()),
            );
        }
        let mut out = String::new();
        for ((path, scope), (n, line)) in groups {
            if !filter.path_ok(&path) {
                continue;
            }
            let scope = if scope.is_empty() {
                String::new()
            } else {
                format!("  {scope}")
            };
            out.push_str(&format!("{path}{scope} ×{n} (L{line})\n"));
        }
        if out.is_empty() {
            out = format!("no references to {name}{}", filter.scope_note());
        }
        finish(cx, "callers", t0, out)
    })
}

pub(crate) fn impact(
    cx: &Ctx,
    root: &Path,
    name: &str,
    depth: u32,
    filter: &super::Filter,
) -> Result<String> {
    let t0 = Instant::now();
    with_session(root, |s| {
        let Some(d) = wait_def(s, name)? else {
            return finish(
                cx,
                "impact",
                t0,
                format!("nothing reaches {name}{}", filter.scope_note()),
            );
        };
        s.did_open(&d.uri)?;
        let items = s.request(
            "textDocument/prepareCallHierarchy",
            json!({"textDocument": {"uri": d.uri}, "position": d.pos}),
        )?;
        let mut frontier = items.as_array().cloned().unwrap_or_default();
        let mut seen = HashSet::from([name.to_string()]);
        let mut out = String::new();
        for dpth in 1..=depth.clamp(1, 4) {
            let mut next = Vec::new();
            for item in &frontier {
                let calls = s.request("callHierarchy/incomingCalls", json!({"item": item}))?;
                for c in calls.as_array().cloned().unwrap_or_default() {
                    let from = &c["from"];
                    let nm = from["name"].as_str().unwrap_or("");
                    if !seen.insert(nm.to_string()) {
                        continue;
                    }
                    let path = rel(&s.root, from["uri"].as_str().unwrap_or(""));
                    if !filter.path_ok(&path) {
                        continue;
                    }
                    if nm.is_empty() {
                        out.push_str(&format!("{dpth}  {path}  (file)\n"));
                    } else {
                        out.push_str(&format!("{dpth}  {path}  {nm}\n"));
                    }
                    next.push(from.clone());
                }
            }
            frontier = next;
            if frontier.is_empty() {
                break;
            }
        }
        if out.is_empty() {
            out = format!("nothing reaches {name}{}", filter.scope_note());
        }
        finish(cx, "impact", t0, out)
    })
}

fn flatten(syms: &[Value], out: &mut String) {
    for s in syms {
        let name = s["name"].as_str().unwrap_or("");
        let line = s["range"]["start"]["line"].as_u64().unwrap_or(0) + 1;
        let prefix = match kind_name(s["kind"].as_u64().unwrap_or(0)) {
            "function" | "method" => "fn",
            other => other,
        };
        out.push_str(&format!("{prefix} {name} {line}\n"));
        if let Some(c) = s["children"].as_array() {
            flatten(c, out);
        }
    }
}

fn workspace_of(file: &Path) -> PathBuf {
    for d in file.ancestors().skip(1) {
        if d.join("Cargo.toml").is_file()
            || d.join("compile_commands.json").is_file()
            || d.join("tsconfig.json").is_file()
            || d.join("pubspec.yaml").is_file()
        {
            return d.to_path_buf();
        }
    }
    file.parent().unwrap_or(file).to_path_buf()
}

pub(crate) fn outline(cx: &Ctx, root: &Path, path: &str) -> Result<String> {
    let t0 = Instant::now();
    let abs = if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        root.join(path)
    };
    // Same session key as `symbol` / `callers` / `impact` (cwd) when the file sits under cwd and
    // its nearest manifest speaks cwd's server; otherwise the nearest manifest. Keyed by the
    // nearest manifest alone, alternating `outline` and `symbol` calls in a Cargo workspace
    // killed and respawned the server each time; keyed by cwd alone, a Dart file outside a
    // Cargo cwd was sent to rust-analyzer and outlined as empty.
    let near = workspace_of(&abs);
    let ws = match (pick(root), pick(&near)) {
        (Ok((a, _)), Ok((b, _))) if a == b && abs.starts_with(root) => root.to_path_buf(),
        _ => near,
    };
    with_session(&ws, |s| {
        let uri = file_uri(&abs);
        s.did_open(&uri)?;
        let deadline = Instant::now() + READY;
        loop {
            let raw = s.request(
                "textDocument/documentSymbol",
                json!({"textDocument": {"uri": uri}}),
            )?;
            let mut text = String::new();
            if let Some(arr) = raw.as_array() {
                flatten(arr, &mut text);
            }
            if !text.is_empty() || Instant::now() >= deadline {
                return finish(cx, "outline", t0, text.trim_end().to_string());
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    /// T41.1: a `pubspec.yaml` root picks `dart language-server`, and a `.dart`
    /// file resolves its workspace to that root.
    #[test]
    fn dart_pubspec_root_picks_dart_language_server() {
        let dir = std::env::temp_dir().join(format!("rtok-lsp-dart-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("lib")).unwrap();
        std::fs::write(dir.join("pubspec.yaml"), "name: dart_gate\n").unwrap();
        let main = dir.join("lib/main.dart");
        std::fs::write(&main, "void main() {}\n").unwrap();
        let (bin, args) = pick(&dir).unwrap();
        assert_eq!(bin, "dart");
        assert_eq!(args, &["language-server"]);
        assert_eq!(workspace_of(&main), dir);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The cached server outlived `rtok mcp`: it sat in a `static`, and statics never drop.
    #[test]
    fn shutdown_reaps_the_cached_server_and_its_stderr_file() {
        let mut child = Command::new("cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let pid = child.id().to_string();
        let err_path = std::env::temp_dir().join(format!("rtok-lsp-test-{pid}.stderr"));
        std::fs::write(&err_path, "").unwrap();
        let s = Session {
            stdin: child.stdin.take().unwrap(),
            stdout: BufReader::new(child.stdout.take().unwrap()),
            child,
            next_id: 1,
            root: PathBuf::new(),
            opened: HashSet::new(),
            err_path: err_path.clone(),
        };
        *SESSION.lock().unwrap_or_else(|e| e.into_inner()) = Some(("test".into(), s));
        shutdown();
        let alive = Command::new("kill").args(["-0", &pid]).status().unwrap();
        assert!(!alive.success(), "server {pid} still running");
        assert!(!err_path.exists());
    }
}

#[cfg(test)]
mod uri_tests {
    use super::{file_uri, path_from_file_uri};
    use std::path::PathBuf;

    #[test]
    fn file_uri_uses_forward_slashes() {
        let dir = std::env::temp_dir().join(format!("rtok-uri-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let uri = file_uri(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(uri.starts_with("file://"), "{uri}");
        assert!(!uri.contains('\\'), "no backslashes: {uri}");
        if cfg!(windows) {
            assert!(
                uri.starts_with("file:///"),
                "windows needs three slashes: {uri}"
            );
        }
        let round = path_from_file_uri(&uri);
        assert!(!round.as_os_str().is_empty(), "{round:?}");
    }

    #[test]
    fn path_from_file_uri_keeps_unix_root() {
        let p = path_from_file_uri("file:///tmp/a");
        assert_eq!(p, PathBuf::from("/tmp/a"));
    }

    #[cfg(windows)]
    #[test]
    fn path_from_file_uri_strips_slash_before_drive() {
        let p = path_from_file_uri("file:///C:/Users/x");
        assert_eq!(p, PathBuf::from(r"C:\Users\x"));
    }

    #[test]
    fn file_uri_encodes_spaces() {
        // Pure encode/decode — no host TempDir (VFS / T56).
        let encoded = super::percent_encode_path("C:/Users/Ivan Tuhai/proj/a.rs");
        assert!(encoded.contains("%20"), "{encoded}");
        assert!(!encoded.contains(' '), "{encoded}");
        assert!(encoded.starts_with("C:"), "{encoded}");
        let uri = format!("file:///{encoded}");
        let round = path_from_file_uri(&uri);
        assert!(round.to_string_lossy().contains("Ivan Tuhai"), "{round:?}");
    }

    #[test]
    fn path_from_file_uri_decodes_percent20() {
        let p = path_from_file_uri("file:///tmp/My%20Docs/a.rs");
        assert_eq!(p, PathBuf::from("/tmp/My Docs/a.rs"));
    }
}
