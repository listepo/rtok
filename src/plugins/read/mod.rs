//! `read` — MCP `read` / `search` / `tree` with modes, size caps and re-read dedup (plan P4).
//!
//! Spec: the catalogue in `plan.md` §1 names the tools this replaces; none is a
//! dependency (D6) — the behaviour is re-implemented here.

use std::path::{Component, Path, PathBuf};

use anyhow::{Result, bail};
use serde_json::{Value, json};

use rtok_plugin_sdk::{
    Ctx, DashboardPage, Manifest, Plugin, PostToolUse, PreToolDecision, PreToolUse, Surface,
    ToolDef,
};

pub mod cache;
pub mod fs;
pub mod hook;
pub(crate) mod outline;
pub mod search;
#[cfg(test)]
pub(crate) mod walk;

pub struct Read;

impl Plugin for Read {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "read",
            surfaces: &[Surface::Mcp, Surface::Hook],
            default_on: true,
        }
    }

    fn dashboard_page(&self) -> DashboardPage {
        DashboardPage::new(
            "Read",
            "Outline, map, and search instead of dumping full files into context.",
            true,
        )
    }

    fn pre_tool(&self, ev: &PreToolUse, cx: &Ctx) -> Option<PreToolDecision> {
        hook::pre_tool(ev, cx)
    }

    fn post_tool(&self, ev: &PostToolUse, cx: &Ctx) -> Option<String> {
        cache::invalidate(ev, cx);
        None
    }

    fn mcp_tools(&self) -> Vec<ToolDef> {
        vec![
            ToolDef {
                name: "read",
                description: "Read a file; mode full|lines|map|signatures|diff; range a-b for full|lines.",
                input_schema: json!({"type":"object","properties":{"path":{"type":"string"},"mode":{"type":"string"},"range":{"type":"string"}},"required":["path"]}),
            },
            ToolDef {
                name: "search",
                description: "Regex search files; path:line: snippet, max hits.",
                input_schema: json!({"type":"object","properties":{"pattern":{"type":"string"},"path":{"type":"string"},"max":{"type":"integer"}},"required":["pattern"]}),
            },
            ToolDef {
                name: "tree",
                description: "Compact directory listing with sizes; depth cap.",
                input_schema: json!({"type":"object","properties":{"path":{"type":"string"},"depth":{"type":"integer"}}}),
            },
        ]
    }
}

pub fn read(cx: &Ctx, path: &str, mode: &str, range: Option<&str>) -> Result<String> {
    let cwd = std::env::current_dir()?;
    read_with(cx, &fs::HostFs, &cwd, path, mode, range)
}

/// The file a Read/Edit/Write input names: Claude Code's `file_path`, or Copilot's
/// `path` (`read_file`/`view` are adapted to the tool name `Read` but keep their key).
pub(crate) fn path_arg(input: &Value) -> Option<&str> {
    input
        .get("file_path")
        .or_else(|| input.get("path"))
        .and_then(Value::as_str)
}

/// `read` over an arbitrary [`fs::ReadFs`] (host disk or [`crate::testutil::Vfs`]).
pub(crate) fn read_with(
    cx: &Ctx,
    fs: &impl fs::ReadFs,
    cwd: &Path,
    path: &str,
    mode: &str,
    range: Option<&str>,
) -> Result<String> {
    let cfg = cx.plugin_config::<crate::config::Read>("read");
    let abs = resolve_with(fs, cwd, Path::new(path), &cfg.allow_paths)?;
    let raw = fs.read_to_string(&abs)?;
    let mode = if mode.is_empty() {
        cfg.default_mode.as_str()
    } else {
        mode
    };
    let force_delta = mode == "diff";
    let mode = if force_delta { "full" } else { mode };
    let body = if mode == "map" || mode == "signatures" {
        outline::render(&abs, &raw, mode)?
    } else {
        // Same grammar as `expand --lines` (`a`, `a-b`, `a-`, `-b`); a malformed range is an
        // error, not the whole file. Outline modes carry their own line numbers per definition.
        let lines: Vec<&str> = raw.lines().collect();
        let (a, b) = match range {
            Some(spec) => crate::expand::parse_range(spec, lines.len())?,
            None => (1, lines.len()),
        };
        crate::expand::slice_lines(lines, a, b)
            .iter()
            .enumerate()
            .map(|(i, l)| format!("{}:{l}", a + i))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let key = cache::key(abs.to_string_lossy().as_ref(), mode, range);
    let payload = if mode == "map" || mode == "signatures" {
        body.as_bytes()
    } else {
        raw.as_bytes()
    };
    if let Some(hit) = cache::hit(
        cx,
        &key,
        abs.to_string_lossy().as_ref(),
        payload,
        body.lines().count(),
        force_delta,
    ) {
        return Ok(hit);
    }
    // Raw file bytes (not the delta view): a same-session content-hash hit is a pointer.
    if let Some(msg) = crate::plugin::identical_result(&**cx, "read", payload) {
        let _ = cache::remember(cx, &key, payload);
        return Ok(msg);
    }
    let _ = cache::remember(cx, &key, payload);
    cap(cx, body)
}

pub(crate) fn resolve(cwd: &Path, path: &Path, extra: &[PathBuf]) -> Result<PathBuf> {
    resolve_with(&fs::HostFs, cwd, path, extra)
}

/// Resolve + allow-check using [`fs::ReadFs::canonicalize`] (symlink follow on host / Vfs).
pub(crate) fn resolve_with(
    fs: &impl fs::ReadFs,
    cwd: &Path,
    path: &Path,
    extra: &[PathBuf],
) -> Result<PathBuf> {
    // Canonicalise roots first. Relative paths are then joined onto the
    // canonical cwd so a missing file (canonicalize fails) still shares the
    // same case as the root `under` compares against.
    let roots: Vec<PathBuf> = std::iter::once(cwd.to_path_buf())
        .chain(extra.iter().cloned())
        .map(|r| fs.canonicalize(&r).unwrap_or(r))
        .collect();
    let abs = if path.is_absolute() {
        normalize(Path::new("/"), path)
    } else {
        normalize(&roots[0], path)
    };
    // Lexical allow, then (when the path exists) reject symlink escapes past the root.
    let check = fs.canonicalize(&abs).unwrap_or_else(|| abs.clone());
    if roots.iter().any(|r| under(&check, r)) {
        return Ok(abs);
    }
    bail!("path outside cwd: {}", path.display())
}

fn normalize(root: &Path, path: &Path) -> PathBuf {
    let mut out = if path.is_absolute() {
        PathBuf::new()
    } else {
        root.to_path_buf()
    };
    for c in path.components() {
        match c {
            // Push, do not replace: on Windows `C:\foo` is Prefix("C:") then
            // RootDir — replacing wiped the drive and confined to `\foo`.
            Component::RootDir => out.push(Component::RootDir.as_os_str()),
            Component::Prefix(p) => out = PathBuf::from(p.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(s) => out.push(s),
        }
    }
    out
}

/// `Path::starts_with("")` is true for every path, so an empty root (`allow_paths = [""]`)
/// would open the whole filesystem; it grants nothing instead.
///
/// On Windows, `Path::starts_with` is case-sensitive even though the filesystem
/// is not. When canonicalize succeeds on the root but fails on a missing path
/// (or the other way around), the two sides can differ only in ASCII case and
/// a lexical `starts_with` wrongly rejects an in-tree file.
fn under(path: &Path, root: &Path) -> bool {
    if root.as_os_str().is_empty() {
        return false;
    }
    if path.starts_with(root) {
        return true;
    }
    cfg!(windows) && under_ascii_case_insensitive(path, root)
}

#[cfg_attr(not(windows), allow(dead_code))]
fn under_ascii_case_insensitive(path: &Path, root: &Path) -> bool {
    use std::path::Component;
    let path_c: Vec<_> = path.components().collect();
    let root_c: Vec<_> = root.components().collect();
    if root_c.is_empty() || root_c.len() > path_c.len() {
        return false;
    }
    path_c.iter().zip(root_c.iter()).all(|(p, r)| match (p, r) {
        (Component::Normal(a), Component::Normal(b)) => a.eq_ignore_ascii_case(b),
        (Component::Prefix(a), Component::Prefix(b)) => {
            a.as_os_str().eq_ignore_ascii_case(b.as_os_str())
        }
        (a, b) => a == b,
    })
}

/// Cap at `plugins.read.max_chars`; an oversized text is archived and the cut carries its id.
pub(crate) fn cap(cx: &Ctx, text: String) -> Result<String> {
    let max = cx.plugin_config::<crate::config::Read>("read").max_chars as usize;
    if text.chars().count() <= max {
        return Ok(text);
    }
    let id = cx.put_archive(text.as_bytes())?;
    Ok(crate::expand::cut(
        &text,
        &format!("\n… archived {id} …\n"),
        max,
    ))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use rstest::rstest;
    use std::fs;

    /// A runtime whose fresh temp dir is also the one `allow_paths` root; shared with `cache.rs`.
    pub(crate) fn cx(name: &str) -> (crate::plugin::Runtime, PathBuf) {
        let (mut c, dir) = crate::testutil::config(name);
        c.plugins.read.allow_paths = vec![dir.clone()];
        (crate::plugin::Runtime::open(c, name).unwrap(), dir)
    }

    // Disk e2e kept; Vfs twins below call `read_with` / `resolve_with` (T56.5 ReadFs).
    #[test]
    fn three_lines_are_numbered() {
        let (cx, dir) = cx("three");
        let p = dir.join("a.txt");
        fs::write(&p, "alpha\nbeta\ngamma\n").unwrap();
        let out = read(&Ctx::new(&cx), p.to_str().unwrap(), "full", None).unwrap();
        assert_eq!(out, "1:alpha\n2:beta\n3:gamma");
        let _ = fs::remove_dir_all(dir);
    }

    /// `range` follows `expand --lines` in every line mode; a malformed one is an error.
    #[test]
    fn range_applies_to_full_and_lines_and_rejects_junk() {
        let (cx, dir) = cx("range");
        let p = dir.join("r.txt");
        fs::write(&p, "a\nb\nc\nd\n").unwrap();
        let path = p.to_str().unwrap();
        let cx = Ctx::new(&cx);
        assert_eq!(read(&cx, path, "full", Some("2-3")).unwrap(), "2:b\n3:c");
        assert_eq!(read(&cx, path, "lines", Some("3")).unwrap(), "3:c\n4:d");
        assert_eq!(read(&cx, path, "lines", Some("-1")).unwrap(), "1:a");
        assert!(read(&cx, path, "lines", Some("x-y")).is_err());
        assert!(read(&cx, path, "full", Some("3-2")).is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[rstest]
    fn hundred_kb_is_capped_with_archive_id() {
        let (cx, dir) = cx("big");
        let max = cx.config.plugins.read.max_chars as usize;
        let p = dir.join("big.txt");
        let blob = "x".repeat(100 * 1024);
        fs::write(&p, &blob).unwrap();
        let out = read(&Ctx::new(&cx), p.to_str().unwrap(), "full", None).unwrap();
        assert!(out.contains("archived"), "{out}");
        assert!(
            out.chars().count() <= max,
            "cap includes marker: {}/{}",
            out.chars().count(),
            max
        );
        assert!(out.chars().count() < blob.len(), "capped");
        let _ = fs::remove_dir_all(dir);
    }

    fn archive_id(out: &str) -> &str {
        const PRE: &str = "… archived ";
        const SUF: &str = " …";
        let start = out.find(PRE).expect("archive marker") + PRE.len();
        let rest = &out[start..];
        &rest[..rest.find(SUF).expect("archive marker end")]
    }

    #[rstest]
    #[case(500)]
    #[case(200)]
    fn cap_includes_marker_in_max_chars(#[case] max_chars: u32) {
        let (mut c, dir) = crate::testutil::config("cap-marker");
        c.plugins.read.allow_paths = vec![dir.clone()];
        c.plugins.read.max_chars = max_chars;
        let cx = crate::plugin::Runtime::open(c, "cap-marker").unwrap();
        let blob = "abcdefghij".repeat(200);
        let p = dir.join("cap.txt");
        fs::write(&p, &blob).unwrap();
        let expected = format!("1:{blob}");
        let out = read(&Ctx::new(&cx), p.to_str().unwrap(), "full", None).unwrap();
        let max = max_chars as usize;
        assert!(
            out.chars().count() <= max,
            "output {} chars exceeds max {}",
            out.chars().count(),
            max
        );
        let id = archive_id(&out);
        let archived = String::from_utf8(cx.store.get_archive(id, None).unwrap().unwrap()).unwrap();
        assert_eq!(archived, expected);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn symlink_escape_is_err() {
        let (_cx, dir) = cx("symlink");
        let cwd = dir.join("cwd");
        fs::create_dir_all(&cwd).unwrap();
        // Target lives beside the allow_paths root, not under it.
        let outside = dir
            .parent()
            .unwrap()
            .join(format!("rtok-read-symlink-out-{}", std::process::id()));
        fs::write(&outside, "secret\n").unwrap();
        let link = cwd.join("escape");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        #[cfg(not(unix))]
        {
            let _ = fs::remove_file(&outside);
            let _ = fs::remove_dir_all(dir);
            return;
        }
        // `read` passes the process cwd to `resolve`; calling `resolve` with `cwd` and no
        // allow_paths checks the same guard without moving the cwd every parallel test shares
        // (moving it failed `map_src_main_lists_fn_main` on ubuntu CI, 2026-09-11).
        let err = resolve(&cwd, Path::new("escape"), &[])
            .unwrap_err()
            .to_string();
        let _ = fs::remove_file(&outside);
        assert!(err.contains("outside cwd"), "{err}");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn normalize_absolute_unix_path_still_roots() {
        let got = normalize(Path::new("/unused"), Path::new("/tmp/a/../b"));
        assert_eq!(got, PathBuf::from("/tmp/b"));
    }

    #[cfg(windows)]
    #[test]
    fn normalize_keeps_windows_drive_prefix() {
        let got = normalize(Path::new(r"C:\unused"), Path::new(r"D:\Users\x\..\y"));
        assert_eq!(got, PathBuf::from(r"D:\Users\y"));
    }

    /// The lexical twin of `symlink_escape_is_err`: `..` climbs past every allowed root.
    #[test]
    fn dot_dot_escape_is_err() {
        let (cx, dir) = cx("dotdot");
        let deep = "../".repeat(32) + "etc/passwd";
        assert!(read(&Ctx::new(&cx), &deep, "full", None).is_err());
        let _ = fs::remove_dir_all(dir);
    }

    /// `RTOK_PLUGINS_READ_ALLOW_PATHS=` once became `[""]`, and `starts_with("")` is always true.
    #[test]
    fn empty_allow_root_grants_nothing() {
        let err = resolve(
            Path::new("/nonexistent-cwd"),
            Path::new("/etc/passwd"),
            &[PathBuf::new()],
        );
        assert!(err.is_err(), "{err:?}");
    }

    /// Missing file + canonical root with different ASCII case: `Path::starts_with`
    /// alone rejects; resolve must still allow (Windows CI path; also covers the
    /// relative rebase onto a canonical cwd on every host).
    #[test]
    fn resolve_allows_missing_path_when_root_case_differs() {
        let base = std::env::temp_dir().join(format!("rtok-read-case-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let proj = base.join("Proj");
        fs::create_dir_all(&proj).unwrap();
        let disk = dunce::canonicalize(&proj).unwrap();
        let mut alt = disk.clone();
        if let Some(name) = alt.file_name().map(|n| n.to_string_lossy().into_owned()) {
            let flipped: String = name
                .chars()
                .map(|c| {
                    if c.is_ascii_uppercase() {
                        c.to_ascii_lowercase()
                    } else if c.is_ascii_lowercase() {
                        c.to_ascii_uppercase()
                    } else {
                        c
                    }
                })
                .collect();
            alt.pop();
            alt.push(flipped);
        }
        // Relative path through the alt-cased cwd string (canonicalize of missing fails).
        let got = resolve(&alt, Path::new("missing.txt"), &[]).expect("under alt cwd");
        assert!(got.ends_with("missing.txt"), "{got:?}");
        // Absolute path with alt case against a canonical root via allow_paths.
        let abs_missing = alt.join("also-missing.txt");
        let got2 = resolve(
            Path::new("/nonexistent-cwd-rtok"),
            &abs_missing,
            std::slice::from_ref(&disk),
        );
        if cfg!(windows) {
            assert!(got2.is_ok(), "{got2:?}");
        } else {
            // On case-sensitive Linux the alt path is a different directory; on
            // macOS the relative rebase above already covers the asymmetry.
            let _ = got2;
        }
        let _ = fs::remove_dir_all(&base);
    }

    #[cfg(windows)]
    #[test]
    fn under_ascii_case_insensitive_matches_windows_prefix() {
        let path = Path::new(r"C:\Users\Me\proj\file.txt");
        let root = Path::new(r"c:\users\me\proj");
        assert_eq!(under_ascii_case_insensitive(path, root), true);
        assert_eq!(
            under_ascii_case_insensitive(path, Path::new(r"c:\users\me\project")),
            false
        );
    }

    /// T56.2: line numbering / range from Vfs bytes — same grammar as `read` full|lines, no host disk.
    fn numbered_from_vfs(raw: &str, range: Option<&str>) -> String {
        let lines: Vec<&str> = raw.lines().collect();
        let (a, b) = match range {
            Some(spec) => crate::expand::parse_range(spec, lines.len()).unwrap(),
            None => (1, lines.len()),
        };
        crate::expand::slice_lines(lines, a, b)
            .iter()
            .enumerate()
            .map(|(i, l)| format!("{}:{l}", a + i))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn three_lines_numbered_from_vfs() {
        let mut vfs = crate::testutil::Vfs::new();
        vfs.write("a.txt", b"alpha\nbeta\ngamma\n");
        let out = numbered_from_vfs(vfs.read_str("a.txt").unwrap(), None);
        assert_eq!(out, "1:alpha\n2:beta\n3:gamma");
    }

    #[test]
    fn range_from_vfs_matches_expand_grammar() {
        let mut vfs = crate::testutil::Vfs::new();
        vfs.write("r.txt", b"a\nb\nc\nd\n");
        let raw = vfs.read_str("r.txt").unwrap();
        assert_eq!(numbered_from_vfs(raw, Some("2-3")), "2:b\n3:c");
        assert_eq!(numbered_from_vfs(raw, Some("3")), "3:c\n4:d");
        assert_eq!(numbered_from_vfs(raw, Some("-1")), "1:a");
    }

    #[test]
    fn range_from_vfs_rejects_junk_like_disk_twin() {
        let mut vfs = crate::testutil::Vfs::new();
        vfs.write("r.txt", b"a\nb\nc\nd\n");
        let raw = vfs.read_str("r.txt").unwrap();
        let lines: Vec<&str> = raw.lines().collect();
        assert!(crate::expand::parse_range("x-y", lines.len()).is_err());
        assert!(crate::expand::parse_range("3-2", lines.len()).is_err());
    }

    #[test]
    fn empty_file_from_vfs_numbers_nothing() {
        let mut vfs = crate::testutil::Vfs::new();
        vfs.write("empty.txt", b"");
        assert_eq!(
            numbered_from_vfs(vfs.read_str("empty.txt").unwrap(), None),
            ""
        );
    }

    #[test]
    fn spaced_path_content_from_vfs_numbers_lines() {
        let mut vfs = crate::testutil::Vfs::new();
        vfs.write("My Docs/notes.txt", b"one\ntwo\n");
        let out = numbered_from_vfs(vfs.read_str("My Docs/notes.txt").unwrap(), Some("1-2"));
        assert_eq!(out, "1:one\n2:two");
    }

    /// T56.5: full `read_with` on Vfs — disk `three_lines_are_numbered` twin (kept).
    #[test]
    fn three_lines_are_numbered_via_read_fs_vfs() {
        let (mut c, dir) = crate::testutil::config("three-vfs");
        c.plugins.read.allow_paths = vec![PathBuf::from("ws")];
        let cx = crate::plugin::Runtime::open(c, "three-vfs").unwrap();
        let mut vfs = crate::testutil::Vfs::new();
        vfs.write("ws/a.txt", b"alpha\nbeta\ngamma\n");
        let out = read_with(&Ctx::new(&cx), &vfs, Path::new("ws"), "a.txt", "full", None).unwrap();
        assert_eq!(out, "1:alpha\n2:beta\n3:gamma");
        let _ = fs::remove_dir_all(dir);
    }

    /// T56.5: range grammar through `read_with` + Vfs (disk twin kept).
    #[test]
    fn range_applies_via_read_fs_vfs() {
        let (mut c, dir) = crate::testutil::config("range-vfs");
        c.plugins.read.allow_paths = vec![PathBuf::from("ws")];
        let cx = crate::plugin::Runtime::open(c, "range-vfs").unwrap();
        let mut vfs = crate::testutil::Vfs::new();
        vfs.write("ws/r.txt", b"a\nb\nc\nd\n");
        let ctx = Ctx::new(&cx);
        assert_eq!(
            read_with(&ctx, &vfs, Path::new("ws"), "r.txt", "full", Some("2-3")).unwrap(),
            "2:b\n3:c"
        );
        assert_eq!(
            read_with(&ctx, &vfs, Path::new("ws"), "r.txt", "lines", Some("3")).unwrap(),
            "3:c\n4:d"
        );
        assert_eq!(
            read_with(&ctx, &vfs, Path::new("ws"), "r.txt", "lines", Some("-1")).unwrap(),
            "1:a"
        );
        assert!(read_with(&ctx, &vfs, Path::new("ws"), "r.txt", "lines", Some("x-y")).is_err());
        assert!(read_with(&ctx, &vfs, Path::new("ws"), "r.txt", "full", Some("3-2")).is_err());
        let _ = fs::remove_dir_all(dir);
    }

    /// T56.5: cap + archive id through `read_with` + Vfs (disk twin kept).
    #[test]
    fn hundred_kb_is_capped_via_read_fs_vfs() {
        let (mut c, dir) = crate::testutil::config("big-vfs");
        c.plugins.read.allow_paths = vec![PathBuf::from("ws")];
        let cx = crate::plugin::Runtime::open(c, "big-vfs").unwrap();
        let max = cx.config.plugins.read.max_chars as usize;
        let mut vfs = crate::testutil::Vfs::new();
        let blob = "x".repeat(100 * 1024);
        vfs.write("ws/big.txt", blob.as_bytes());
        let out = read_with(
            &Ctx::new(&cx),
            &vfs,
            Path::new("ws"),
            "big.txt",
            "full",
            None,
        )
        .unwrap();
        assert!(out.contains("archived"), "{out}");
        assert!(
            out.chars().count() <= max,
            "cap includes marker: {}/{}",
            out.chars().count(),
            max
        );
        assert!(out.chars().count() < blob.len(), "capped");
        let _ = fs::remove_dir_all(dir);
    }

    #[rstest]
    #[case(500)]
    #[case(200)]
    fn cap_includes_marker_via_read_fs_vfs(#[case] max_chars: u32) {
        let (mut c, dir) = crate::testutil::config("cap-marker-vfs");
        c.plugins.read.allow_paths = vec![PathBuf::from("ws")];
        c.plugins.read.max_chars = max_chars;
        let cx = crate::plugin::Runtime::open(c, "cap-marker-vfs").unwrap();
        let blob = "abcdefghij".repeat(200);
        let mut vfs = crate::testutil::Vfs::new();
        vfs.write("ws/cap.txt", blob.as_bytes());
        let expected = format!("1:{blob}");
        let out = read_with(
            &Ctx::new(&cx),
            &vfs,
            Path::new("ws"),
            "cap.txt",
            "full",
            None,
        )
        .unwrap();
        let max = max_chars as usize;
        assert!(
            out.chars().count() <= max,
            "output {} chars exceeds max {}",
            out.chars().count(),
            max
        );
        let id = archive_id(&out);
        let archived = String::from_utf8(cx.store.get_archive(id, None).unwrap().unwrap()).unwrap();
        assert_eq!(archived, expected);
        let _ = fs::remove_dir_all(dir);
    }

    /// T56.5: symlink escape via `resolve_with` + Vfs (disk twin kept).
    #[test]
    fn symlink_escape_via_read_fs_vfs() {
        let mut vfs = crate::testutil::Vfs::new();
        vfs.write("/outside/secret", b"secret\n");
        vfs.symlink("ws/escape", "/outside/secret");
        let err = resolve_with(&vfs, Path::new("ws"), Path::new("escape"), &[])
            .unwrap_err()
            .to_string();
        assert!(err.contains("outside cwd"), "{err}");
    }

    #[test]
    fn map_src_main_lists_fn_main() {
        let (cx, dir) = crate::testutil::runtime("mapmain");
        let out = read(&Ctx::new(&cx), "src/main.rs", "map", None).unwrap();
        assert!(out.contains("fn main"), "{out}");
        let _ = fs::remove_dir_all(dir);
    }
}
