//! Deny duplicate Read/Bash when a prior archive id exists (plan T2.6).

use rtok_plugin_sdk::{
    Ctx, DashboardPage, Manifest, Measurement, Plugin, PostToolUse, PreToolDecision, PreToolUse,
    Surface,
};
use serde_json::Value;

mod skill;

pub struct Guard;

impl Plugin for Guard {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "guard",
            surfaces: &[Surface::Hook, Surface::Cli],
            default_on: true,
        }
    }

    fn dashboard_page(&self) -> DashboardPage {
        DashboardPage::new(
            "Guard",
            "Deny duplicate reads and commands inside a sliding window.",
            true,
        )
    }

    fn pre_tool(&self, ev: &PreToolUse, cx: &Ctx) -> Option<PreToolDecision> {
        if ev.tool_name == "Skill" {
            return skill::digest(ev, cx);
        }
        if let Some(d) = native_redirect(ev.tool_name, cx) {
            return Some(d);
        }
        let key = cache_key(ev.tool_name, ev.tool_input)?;
        let (id, ts) = cx.get_read_cache(&key).ok().flatten()?;
        let id = id?;
        let n = cx.calls_since(ts).unwrap_or(0);
        let window = cx
            .plugin_config::<crate::config::Guard>("guard")
            .window_turns;
        if n > i64::from(window) {
            return None;
        }
        let reason = format!("duplicate; rtok expand {id}");
        // AGENTS: denial Measurement carries the avoided result size (archive bytes).
        // Never deny without a retrievable original (lossless + fail open). Metadata only:
        // reading a possibly-megabyte body would break the ≤ 10 ms hook budget (T55.16).
        let avoided = cx.archive_size(&id).ok().flatten()?;
        if avoided == 0 {
            return None;
        }
        // The same bytes/4 heuristic `record_context_path` and the semantic-cache
        // measurement use — the body is not loaded to estimate it.
        let est = (avoided / 4).max(1) as u32;
        let _ = cx.record(&Measurement {
            plugin: "guard",
            kind: "guard",
            before_bytes: avoided,
            after_bytes: 0,
            est_before: est,
            est_after: 0,
            ref_id: Some(id.clone()),
            call_id: None,
        });
        Some(PreToolDecision::Deny { reason })
    }

    fn post_tool(&self, ev: &PostToolUse, cx: &Ctx) -> Option<String> {
        match cache_key(ev.tool_name, ev.tool_input) {
            Some(key) => {
                let body = payload(ev.tool_response);
                let id = cx.put_archive(&body).ok()?;
                let _ = cx.put_read_cache(&key, &id, Some(&id));
            }
            // A mutating Bash, Edit or Write can change what any earlier command printed
            // or any earlier Read returned. The guard owns its keys (T55.8): a mutating
            // Bash drops every `bash\t…` and `read\t…` key (prefix clears); an Edit or
            // Write drops the bash keys plus its own path's `read\t{path}` key — no
            // dependency on the `read` plugin's invalidation.
            None if matches!(ev.tool_name, "Bash" | "Edit" | "Write") => {
                let _ = cx.clear_read_cache("bash");
                let mutated_path = ev
                    .tool_input
                    .get("file_path")
                    .or_else(|| ev.tool_input.get("path"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|p| !p.is_empty());
                match (ev.tool_name, mutated_path) {
                    ("Bash", _) => {
                        let _ = cx.clear_read_cache("read");
                    }
                    (_, Some(p)) => {
                        let _ = cx.clear_read_cache(&format!("read\t{p}"));
                    }
                    // An Edit/Write whose path is missing can name any file: drop them all.
                    (_, None) => {
                        let _ = cx.clear_read_cache("read");
                    }
                }
            }
            None => {}
        }
        None
    }
}

/// `rtok guard check` — same allow/deny `pre_tool` returns, as a JSON line.
pub fn check(tool: &str, raw_input: &str, cx: &crate::plugin::Runtime) -> String {
    let tool = crate::hooks::types::canonical_tool_name(tool);
    let mut input: Value = serde_json::from_str(raw_input).unwrap_or(Value::Null);
    if let Some(obj) = input.as_object_mut() {
        if let Some(fp) = obj.remove("filePath") {
            obj.entry("file_path").or_insert(fp);
        }
    }
    let ev = PreToolUse {
        tool_name: &tool,
        tool_input: &input,
    };
    match Guard.pre_tool(&ev, &Ctx::new(cx)) {
        Some(PreToolDecision::Deny { reason }) if !reason.is_empty() => {
            serde_json::json!({ "allow": false, "reason": reason }).to_string()
        }
        _ => serde_json::json!({ "allow": true }).to_string(),
    }
}

/// T50.4: opt-in deny of native `Grep`/`Glob` pointing at MCP `search`/`tree`.
/// Fail open: off by default, and silent while the `read` plugin is disabled
/// (no `search`/`tree` to point at). The knob is per-host opt-in, so a host
/// without `rtok mcp` never turns it on; the hook path does no filesystem
/// reads to check the host config, the read-plugin flag is the guard.
fn native_redirect(tool: &str, cx: &Ctx) -> Option<PreToolDecision> {
    let target = match tool {
        "Grep" => "search",
        "Glob" => "tree",
        _ => return None,
    };
    if !cx
        .plugin_config::<crate::config::Guard>("guard")
        .deny_grep_glob
    {
        return None;
    }
    if !cx.plugin_config::<crate::config::Read>("read").enabled {
        return None;
    }
    let reason = format!("native {tool} is disabled here; use rtok {target} (MCP) instead");
    // Countable but claims no saving: the denied output was never seen (D3).
    let _ = cx.record(&Measurement {
        plugin: "guard",
        kind: "native_deny",
        before_bytes: 0,
        after_bytes: 0,
        est_before: 0,
        est_after: 0,
        ref_id: None,
        call_id: None,
    });
    Some(PreToolDecision::Deny { reason })
}

fn cache_key(tool: &str, input: &Value) -> Option<String> {
    match tool {
        // Claude Code sends `file_path`; Copilot's `read_file`/`view` are adapted to the
        // tool name `Read` but keep their own input key `path` — either names the file.
        // The `read\t` prefix is what the mutating arm below clears in one store call
        // (`clear_read_cache` deletes `x` and every `x\t…`).
        "Read" => {
            let p = input
                .get("file_path")
                .or_else(|| input.get("path"))?
                .as_str()?
                .trim();
            (!p.is_empty()).then(|| format!("read\t{p}"))
        }
        "Bash" => {
            let c = norm_cmd(input.get("command")?.as_str()?);
            // Only read-only commands are keyed: a repeat of `cargo test` after an Edit is
            // new information, not a duplicate.
            read_only(&c).then(|| format!("bash\t{c}"))
        }
        _ => None,
    }
}

/// Stems whose output only changes when something else ran in between. Looks past the
/// `cd <dir> &&` prefix [`norm_cmd`] keeps: only the command after it is keyed.
fn read_only(cmd: &str) -> bool {
    let cmd = after_cd_prefix(cmd);
    if has_writer_marker(&cmd) {
        return false;
    }
    cmd.split('|').all(|seg| read_only_stem(seg.trim()))
}

fn read_only_stem(cmd: &str) -> bool {
    let mut w = cmd.split_whitespace();
    match super::cmd::formatters::cmd_stem(w.next().unwrap_or("")) {
        "ls" | "cat" | "head" | "tail" | "grep" | "rg" | "find" | "tree" | "wc" | "sed"
        | "jq" | "awk" => true,
        "git" => matches!(
            w.next(),
            Some("status" | "log" | "diff" | "show" | "branch" | "rev-parse")
        ),
        "cargo" => matches!(w.next(), Some("metadata")),
        _ => false,
    }
}

fn has_writer_marker(cmd: &str) -> bool {
    let tokens: Vec<&str> = cmd.split_whitespace().collect();
    if tokens.windows(2).any(|w| w[0] == "|" && w[1] == "tee") {
        return true;
    }
    for (i, tok) in tokens.iter().enumerate() {
        if tok.starts_with('>') {
            return true;
        }
        if *tok == "-delete" || *tok == "-exec" {
            return true;
        }
        if *tok == "-i" || *tok == "--in-place" {
            return true;
        }
        if *tok == "-f" && i > 0 && super::cmd::formatters::cmd_stem(tokens[0]) == "tail" {
            return true;
        }
    }
    let parts: Vec<&str> = cmd.split('|').collect();
    if parts.len() > 1 {
        for seg in parts.iter().skip(1) {
            if !read_only_stem(seg.trim()) {
                return true;
            }
        }
    }
    false
}

/// Normalized Bash key body: whitespace collapsed, the `rtok run --` wrap stripped,
/// leading `cd … &&` hops folded to the *last* hop's target (T55.9 — the key keeps the
/// directory the rest of the command runs from: relative paths and `git status` differ
/// per directory, so `cd a && ls` ≠ `ls` ≠ `cd b && ls`, and `cd a && cd a && ls`
/// = `cd a && ls`). Quote-aware: `cd 'a && b' && ls` is one hop to `'a && b'`.
fn norm_cmd(s: &str) -> String {
    let t = collapse(s);
    let t = strip_wrap(&t);
    let (dir, rest) = fold_cd(&t);
    match dir {
        Some(d) => format!("cd {d} && {rest}"),
        None => rest,
    }
}

/// Folds leading `cd <dir> &&` hops to the last hop's target. Each remainder re-runs
/// `strip_wrap` because PostToolUse re-wraps the command the model actually ran.
fn fold_cd(s: &str) -> (Option<String>, String) {
    let mut dir = None;
    let mut t = s.to_string();
    while let Some((d, rest)) = strip_cd_hop(&t) {
        dir = Some(d);
        t = strip_wrap(&rest);
    }
    (dir, t)
}

/// One `cd <dir> &&` prefix: the target word (quotes kept as typed) and the remainder
/// after `&&`. `None` unless `&&` follows the target at top level — a quoted path with
/// `&&` inside is one word, not a split point.
fn strip_cd_hop(s: &str) -> Option<(String, String)> {
    let after = s.strip_prefix("cd ")?;
    let rest = crate::plugins::skip_word(after)?;
    let target = after[..after.len() - rest.len()].trim_end();
    if target.is_empty() {
        return None;
    }
    let rest = rest.strip_prefix("&&")?.trim_start();
    Some((target.to_string(), rest.to_string()))
}

/// Strips the `cd <dir> &&` prefix [`norm_cmd`] folded in, so the read-only stem check
/// sees the command that actually runs (`cd a && ls` → `ls`).
fn after_cd_prefix(s: &str) -> &str {
    let after = match s.strip_prefix("cd ") {
        Some(a) => a,
        None => return s,
    };
    match crate::plugins::skip_word(after).and_then(|r| r.strip_prefix("&&")) {
        Some(rest) => rest.trim_start(),
        None => s,
    }
}

/// PreToolUse sees the user's command; PostToolUse often sees `rtok run -- '…'`.
fn strip_wrap(s: &str) -> String {
    let s = s.strip_prefix("rtok run -- ").unwrap_or(s);
    if s.len() >= 2 && s.starts_with('\'') && s.ends_with('\'') {
        let inner = &s[1..s.len() - 1];
        // POSIX sh_quote embedding, or PowerShell doubled single-quotes (T55.4).
        if inner.contains("'\"'\"'") {
            inner.replace("'\"'\"'", "'")
        } else {
            inner.replace("''", "'")
        }
    } else {
        s.to_string()
    }
}

fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn payload(v: &Value) -> Vec<u8> {
    if let Some(s) = v.as_str() {
        return s.as_bytes().to_vec();
    }
    for k in ["stdout", "content"] {
        if let Some(s) = v.get(k).and_then(Value::as_str) {
            return s.as_bytes().to_vec();
        }
    }
    serde_json::to_vec(v).unwrap_or_default()
}


/// `rtok guard check` — same verdict as the hook path (T70.5).
pub fn check_json(cfg: &crate::config::Config, tool: &str, input: &serde_json::Value) -> serde_json::Value {
    use rtok_plugin_sdk::PreToolUse;
    let cx = match crate::plugin::Runtime::open(cfg.clone(), format!("guard-check-{}", std::process::id())) {
        Ok(c) => c,
        Err(_) => return serde_json::json!({"decision": "allow"}),
    };
    let ev = PreToolUse {
        tool_name: tool,
        tool_input: input,
    };
    match Guard.pre_tool(&ev, &crate::plugin::Ctx::new(&cx)) {
        Some(PreToolDecision::Deny { reason }) => serde_json::json!({"decision": "deny", "reason": reason}),
        Some(PreToolDecision::Rewrite { input, reason }) => {
            serde_json::json!({"decision": "rewrite", "input": input, "reason": reason})
        }
        None => serde_json::json!({"decision": "allow"}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtok_plugin_sdk::Ctx;
    use serde_json::json;

    fn setup() -> crate::plugin::Runtime {
        crate::testutil::runtime("guard").0
    }

    /// T55.9: the Bash key keeps the effective `cd` target — relative paths and
    /// `git status` differ per directory, so a repeat behind a `cd` is new information.
    #[test]
    fn bash_key_keeps_the_cd_target() {
        let k = |c: &str| cache_key("Bash", &json!({"command": c}));
        assert_eq!(k("ls"), Some("bash\tls".to_string()));
        assert_eq!(k("cd a && ls"), Some("bash\tcd a && ls".to_string()));
        assert_eq!(k("cd b && ls"), Some("bash\tcd b && ls".to_string()));
        // Consecutive hops fold to the last one (that is where the command runs).
        assert_eq!(
            k("cd a && cd b && ls"),
            Some("bash\tcd b && ls".to_string())
        );
        assert_eq!(
            k("cd a && cd a && ls"),
            Some("bash\tcd a && ls".to_string())
        );
        // A quoted path with `&&` inside is one target word, not a split point.
        assert_eq!(
            k("cd 'a && b' && ls"),
            Some("bash\tcd 'a && b' && ls".to_string())
        );
        // Whitespace normalization survives the fold.
        assert_eq!(k("cd   a   &&   ls"), k("cd a && ls"));
    }

    /// T55.9 rewrite of the cwd-blind pin: the same command behind a `cd` is a new
    /// key (no deny); the same `cd` + command still denies as a duplicate.
    #[test]
    fn bash_repeat_behind_cd_prefix_is_a_new_key() {
        let cx = setup();
        let g = Guard;
        let first = json!({"command": "ls   -la"});
        let resp = json!({"stdout": "total 3"});
        let post = PostToolUse {
            tool_name: "Bash",
            tool_input: &first,
            tool_response: &resp,
        };
        assert!(g.post_tool(&post, &Ctx::new(&cx)).is_none());
        let pre = |input| PreToolUse {
            tool_name: "Bash",
            tool_input: input,
        };
        let behind_cd = json!({"command": "cd /repo && cd sub && ls -la"});
        assert!(
            g.pre_tool(&pre(&behind_cd), &Ctx::new(&cx)).is_none(),
            "a repeat from another directory is new information, not a duplicate"
        );
        let again = json!({"command": "ls -la"});
        assert!(
            matches!(
                g.pre_tool(&pre(&again), &Ctx::new(&cx)),
                Some(PreToolDecision::Deny { .. })
            ),
            "the same directory-less repeat is still a duplicate"
        );
        let same_hop = g.post_tool(
            &PostToolUse {
                tool_name: "Bash",
                tool_input: &behind_cd,
                tool_response: &resp,
            },
            &Ctx::new(&cx),
        );
        assert!(same_hop.is_none());
        let folded = json!({"command": "cd sub && ls -la"});
        assert!(
            matches!(
                g.pre_tool(&pre(&folded), &Ctx::new(&cx)),
                Some(PreToolDecision::Deny { .. })
            ),
            "same effective directory + command is the same key"
        );
        let other = json!({"command": "ls -l"});
        assert!(g.pre_tool(&pre(&other), &Ctx::new(&cx)).is_none());
    }

    /// `cargo test` is never keyed, and a mutating Bash, Edit or Write drops every Bash key.
    #[test]
    fn mutation_between_commands_allows_the_repeat() {
        let cx = setup();
        let g = Guard;
        let resp = json!({"stdout": "out"});
        let post = |name: &'static str, input: &Value| {
            let ev = PostToolUse {
                tool_name: name,
                tool_input: input,
                tool_response: &resp,
            };
            assert!(g.post_tool(&ev, &Ctx::new(&cx)).is_none());
        };
        let denied = |input: &Value| {
            let ev = PreToolUse {
                tool_name: "Bash",
                tool_input: input,
            };
            matches!(
                g.pre_tool(&ev, &Ctx::new(&cx)),
                Some(PreToolDecision::Deny { .. })
            )
        };
        let test = json!({"command": "cargo test"});
        post("Bash", &test);
        assert!(!denied(&test), "cargo test is not read-only");
        let cat = json!({"command": "cat a.txt"});
        post("Bash", &cat);
        assert!(denied(&cat));
        post("Bash", &test);
        assert!(!denied(&cat), "a mutating Bash clears bash keys");
        post("Bash", &cat);
        assert!(denied(&cat));
        post("Edit", &json!({"file_path": "/proj/b.rs"}));
        assert!(!denied(&cat), "an Edit clears bash keys");
        post("Bash", &json!({"command": "git status"}));
        assert!(denied(&json!({"command": "git status"})));
        assert!(!denied(&json!({"command": "git add ."})));
    }

    #[test]
    fn two_identical_reads_second_denies_naming_archive() {
        let cx = setup();
        let g = Guard;
        let path = json!({"file_path": "/Users/dev/proj/src/main.rs"});
        let other = json!({"file_path": "/Users/dev/proj/src/lib.rs"});
        let read = PreToolUse {
            tool_name: "Read",
            tool_input: &path,
        };
        assert!(g.pre_tool(&read, &Ctx::new(&cx)).is_none());
        let resp = json!({"content": "fn main() {}"});
        let post = PostToolUse {
            tool_name: "Read",
            tool_input: &path,
            tool_response: &resp,
        };
        assert!(g.post_tool(&post, &Ctx::new(&cx)).is_none());
        match g.pre_tool(&read, &Ctx::new(&cx)) {
            Some(PreToolDecision::Deny { reason }) => {
                assert!(reason.contains("rtok expand "), "{reason}");
                let id = reason.rsplit(' ').next().unwrap();
                assert!(reason.contains(id));
            }
            other => panic!("{other:?}"),
        }
        let diff = PreToolUse {
            tool_name: "Read",
            tool_input: &other,
        };
        assert!(g.pre_tool(&diff, &Ctx::new(&cx)).is_none());
        assert!(cx.store.measurement_count("guard").unwrap() >= 1);
        let rows = cx.store.list_measurements("guard").unwrap();
        assert!(rows.iter().any(|r| r.before_bytes > 0), "{rows:?}");
    }

    #[test]
    fn edit_clears_guard_read_so_the_next_read_is_allowed() {
        let cx = setup();
        let g = Guard;
        let path = json!({"file_path": "/proj/src/main.rs"});
        let resp = json!({"content": "fn main() {}"});
        assert!(
            g.post_tool(
                &PostToolUse {
                    tool_name: "Read",
                    tool_input: &path,
                    tool_response: &resp,
                },
                &Ctx::new(&cx),
            )
            .is_none()
        );
        crate::plugins::read::cache::invalidate(
            &PostToolUse {
                tool_name: "Edit",
                tool_input: &path,
                tool_response: &json!({}),
            },
            &Ctx::new(&cx),
        );
        let read = PreToolUse {
            tool_name: "Read",
            tool_input: &path,
        };
        assert!(g.pre_tool(&read, &Ctx::new(&cx)).is_none());
    }

    #[test]
    fn missing_archive_fails_open() {
        let cx = setup();
        let g = Guard;
        let path = json!({"file_path": "/proj/gone.rs"});
        let resp = json!({"content": "old"});
        assert!(
            g.post_tool(
                &PostToolUse {
                    tool_name: "Read",
                    tool_input: &path,
                    tool_response: &resp,
                },
                &Ctx::new(&cx),
            )
            .is_none()
        );
        let key = cache_key("Read", &path).unwrap();
        let (id, _) = cx.store.get_read_cache(&cx.session, &key).unwrap().unwrap();
        let id = id.unwrap();
        std::fs::remove_file(cx.config.core.archive_dir.join(&id)).unwrap();
        let read = PreToolUse {
            tool_name: "Read",
            tool_input: &path,
        };
        assert!(g.pre_tool(&read, &Ctx::new(&cx)).is_none());
    }

    /// T50.4: off by default, Grep points at `search` and Glob at `tree` when on,
    /// and a disabled `read` plugin fails open (no MCP tools to point at).
    #[test]
    fn native_grep_glob_deny_is_opt_in_and_points_at_mcp() {
        let g = Guard;
        let empty = json!({});
        let pre = |tool: &'static str| PreToolUse {
            tool_name: tool,
            tool_input: &empty,
        };
        // Default config: the knob is off, everything passes through.
        let cx = setup();
        assert!(g.pre_tool(&pre("Grep"), &Ctx::new(&cx)).is_none());
        assert!(g.pre_tool(&pre("Glob"), &Ctx::new(&cx)).is_none());
        // Knob on: Grep and Glob deny naming their MCP replacement.
        let (mut c, _dir) = crate::testutil::config("grep-glob");
        c.plugins.guard.deny_grep_glob = true;
        let cx = crate::plugin::Runtime::open(c, "grep-glob").unwrap();
        match g.pre_tool(&pre("Grep"), &Ctx::new(&cx)) {
            Some(PreToolDecision::Deny { reason }) => {
                assert!(reason.contains("search"), "{reason}")
            }
            other => panic!("{other:?}"),
        }
        match g.pre_tool(&pre("Glob"), &Ctx::new(&cx)) {
            Some(PreToolDecision::Deny { reason }) => assert!(reason.contains("tree"), "{reason}"),
            other => panic!("{other:?}"),
        }
        // Other tools are untouched, and the deny is counted without claiming bytes.
        assert!(g.pre_tool(&pre("Read"), &Ctx::new(&cx)).is_none());
        let rows = cx.store.list_measurements("guard").unwrap();
        assert!(
            rows.iter()
                .any(|r| r.kind == "native_deny" && r.before_bytes == 0 && r.after_bytes == 0),
            "{rows:?}"
        );
        // Read plugin disabled: no search/tree to point at, so allow (fail open).
        let (mut c, _dir) = crate::testutil::config("grep-noread");
        c.plugins.guard.deny_grep_glob = true;
        c.plugins.read.enabled = false;
        let cx = crate::plugin::Runtime::open(c, "grep-noread").unwrap();
        assert!(g.pre_tool(&pre("Grep"), &Ctx::new(&cx)).is_none());
        assert!(g.pre_tool(&pre("Glob"), &Ctx::new(&cx)).is_none());
    }

    /// T55.13: Copilot's adapted `Read` keeps its own input key `path`; the deny must
    /// fire for it exactly as it does for Claude Code's `file_path`.
    #[test]
    fn copilot_path_key_dedups_like_file_path() {
        let cx = setup();
        let g = Guard;
        let resp = json!({"content": "fn main() {}"});
        for key in ["path", "file_path"] {
            let input = json!({ key: "/Users/dev/proj/src/main.rs" });
            assert!(
                g.post_tool(
                    &PostToolUse {
                        tool_name: "Read",
                        tool_input: &input,
                        tool_response: &resp,
                    },
                    &Ctx::new(&cx),
                )
                .is_none()
            );
            match g.pre_tool(
                &PreToolUse {
                    tool_name: "Read",
                    tool_input: &input,
                },
                &Ctx::new(&cx),
            ) {
                Some(PreToolDecision::Deny { reason }) => {
                    assert!(reason.contains("rtok expand "), "{key}: {reason}")
                }
                other => panic!("{key}: {other:?}"),
            }
        }
    }

    /// T55.8: a mutating Bash between two identical Reads must not serve the stale
    /// archive — the guard drops its own `read\t…` keys, no `read` plugin involved.
    #[test]
    fn bash_mutation_allows_the_next_read() {
        let cx = setup();
        let g = Guard;
        let path = json!({"file_path": "/proj/src/main.rs"});
        let resp = json!({"content": "fn main() {}"});
        let post = |name: &'static str, input: &Value| {
            assert!(
                g.post_tool(
                    &PostToolUse {
                        tool_name: name,
                        tool_input: input,
                        tool_response: &resp,
                    },
                    &Ctx::new(&cx),
                )
                .is_none()
            );
        };
        post("Read", &path);
        let read = PreToolUse {
            tool_name: "Read",
            tool_input: &path,
        };
        assert!(matches!(
            g.pre_tool(&read, &Ctx::new(&cx)),
            Some(PreToolDecision::Deny { .. })
        ));
        post("Bash", &json!({"command": "cargo fmt"}));
        assert!(
            g.pre_tool(&read, &Ctx::new(&cx)).is_none(),
            "a mutating Bash must drop the guard read key"
        );
    }

    /// T55.8: Edit drops that path's guard read key even with the `read` plugin off.
    #[test]
    fn edit_with_read_plugin_off_allows_the_next_read() {
        let (mut c, _dir) = crate::testutil::config("guard-edit-noread");
        c.plugins.read.enabled = false;
        let cx = crate::plugin::Runtime::open(c, "guard-edit-noread").unwrap();
        let g = Guard;
        let path = json!({"file_path": "/proj/src/main.rs"});
        let resp = json!({"content": "fn main() {}"});
        for name in ["Read", "Edit"] {
            assert!(
                g.post_tool(
                    &PostToolUse {
                        tool_name: name,
                        tool_input: &path,
                        tool_response: &resp,
                    },
                    &Ctx::new(&cx),
                )
                .is_none()
            );
        }
        let read = PreToolUse {
            tool_name: "Read",
            tool_input: &path,
        };
        assert!(
            g.pre_tool(&read, &Ctx::new(&cx)).is_none(),
            "guard's own Edit invalidation must not need the read plugin"
        );
    }

    /// T55.16: the deny reads metadata only — an unreadable body file still denies
    /// (the old body read would have failed open), so megabytes never cross the
    /// ≤ 10 ms hook path.
    #[cfg(unix)]
    #[test]
    fn deny_does_not_read_the_archive_body() {
        use std::os::unix::fs::PermissionsExt;
        let cx = setup();
        let g = Guard;
        let path = json!({"file_path": "/proj/src/hot.rs"});
        let resp = json!({"content": "fn main() {}"});
        assert!(
            g.post_tool(
                &PostToolUse {
                    tool_name: "Read",
                    tool_input: &path,
                    tool_response: &resp,
                },
                &Ctx::new(&cx),
            )
            .is_none()
        );
        let key = cache_key("Read", &path).unwrap();
        let (id, _) = cx.store.get_read_cache(&cx.session, &key).unwrap().unwrap();
        let id = id.unwrap();
        let file = cx.config.core.archive_dir.join(&id);
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o000)).unwrap();
        let read = PreToolUse {
            tool_name: "Read",
            tool_input: &path,
        };
        assert!(
            matches!(
                g.pre_tool(&read, &Ctx::new(&cx)),
                Some(PreToolDecision::Deny { .. })
            ),
            "an unreadable body must still deny — only its metadata was consulted"
        );
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
    }

    #[test]
    fn read_only_flag_aware_stems() {
        assert!(read_only("sed -n '1,40p' file"));
        assert!(!read_only("sed -i 's/a/b/' file"));
        assert!(!read_only("find . -name x -delete"));
        assert!(!read_only("cat a > b"));
        assert!(!read_only("tail -f log"));
        assert!(read_only("cat a | grep b"));
        assert!(!read_only("ls | xargs rm"));
    }

    #[test]
    fn mutating_bash_clears_then_allows_repeat_ls() {
        let cx = setup();
        let g = Guard;
        let ctx = || Ctx::new(&cx);
        let ls = json!({"command": "ls"});
        let ls_resp = json!({"stdout": "a"});
        assert!(g
            .post_tool(
                &PostToolUse {
                    tool_name: "Bash",
                    tool_input: &ls,
                    tool_response: &ls_resp,
                },
                &ctx(),
            )
            .is_none());
        let mutating = json!({"command": "find . -delete"});
        assert!(g
            .post_tool(
                &PostToolUse {
                    tool_name: "Bash",
                    tool_input: &mutating,
                    tool_response: &json!({"stdout": ""}),
                },
                &ctx(),
            )
            .is_none());
        assert!(g
            .pre_tool(
                &PreToolUse {
                    tool_name: "Bash",
                    tool_input: &ls,
                },
                &ctx(),
            )
            .is_none());
    }

    #[test]
    fn wrapped_bash_post_matches_unwrapped_pre() {
        let cx = setup();
        let g = Guard;
        let quoted = format!(
            "rtok run -- {}",
            crate::plugins::cmd::run::sh_quote("git status")
        );
        let wrapped = json!({"command": quoted});
        let resp = json!({"stdout": "ok"});
        assert!(
            g.post_tool(
                &PostToolUse {
                    tool_name: "Bash",
                    tool_input: &wrapped,
                    tool_response: &resp,
                },
                &Ctx::new(&cx),
            )
            .is_none()
        );
        let pre = json!({"command": "git status"});
        assert!(matches!(
            g.pre_tool(
                &PreToolUse {
                    tool_name: "Bash",
                    tool_input: &pre,
                },
                &Ctx::new(&cx),
            ),
            Some(PreToolDecision::Deny { .. })
        ));
    }
}
