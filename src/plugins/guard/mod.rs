//! Deny duplicate Read/Bash when a prior archive id exists (plan T2.6).

use rtok_plugin_sdk::{
    Class, Ctx, DashboardPage, Manifest, Measurement, Plugin, PostToolUse, PreToolDecision,
    PreToolUse, Surface,
};
use serde_json::Value;

pub struct Guard;

impl Plugin for Guard {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "guard",
            surfaces: &[Surface::Hook],
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
        // Never deny without a retrievable original (lossless + fail open).
        let body = cx.get_archive(&id).ok().flatten()?;
        if body.is_empty() {
            return None;
        }
        let avoided = body.len() as u64;
        let est = cx.estimate(&String::from_utf8_lossy(&body), Class::Code);
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
            // A mutating Bash, Edit or Write can change what any earlier command prints:
            // drop every `bash\t…` key (the store deletes by the `bash` prefix).
            None if matches!(ev.tool_name, "Bash" | "Edit" | "Write") => {
                let _ = cx.clear_read_cache("bash");
            }
            None => {}
        }
        None
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
        "Read" => {
            let p = input.get("file_path")?.as_str()?.trim();
            (!p.is_empty()).then(|| format!("read:{p}"))
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

/// Stems whose output only changes when something else ran in between.
fn read_only(cmd: &str) -> bool {
    let mut w = cmd.split_whitespace();
    match super::cmd::formatters::cmd_stem(w.next().unwrap_or("")) {
        "ls" | "cat" | "head" | "tail" | "grep" | "rg" | "find" | "tree" | "wc" => true,
        "git" => matches!(
            w.next(),
            Some("status" | "log" | "diff" | "show" | "branch")
        ),
        _ => false,
    }
}

fn norm_cmd(s: &str) -> String {
    let mut t = collapse(s);
    t = strip_wrap(&t);
    while let Some(rest) = strip_cd_and(&t) {
        t = strip_wrap(&rest);
    }
    t
}

/// PreToolUse sees the user's command; PostToolUse often sees `rtok run -- '…'`.
fn strip_wrap(s: &str) -> String {
    let s = s.strip_prefix("rtok run -- ").unwrap_or(s);
    if s.len() >= 2 && s.starts_with('\'') && s.ends_with('\'') {
        s[1..s.len() - 1].replace("'\"'\"'", "'")
    } else {
        s.to_string()
    }
}

fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn strip_cd_and(s: &str) -> Option<String> {
    let s = s.strip_prefix("cd ")?;
    let i = s.find("&&")?;
    Some(collapse(&s[i + 2..]))
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

#[cfg(test)]
mod tests {
    use super::*;
    use rtok_plugin_sdk::Ctx;
    use serde_json::json;

    fn setup() -> crate::plugin::Runtime {
        crate::testutil::runtime("guard").0
    }

    /// The Bash key collapses whitespace and strips leading `cd … &&` hops, so the same
    /// command run from a `cd` prefix is the same result.
    #[test]
    fn bash_repeat_behind_cd_prefix_denies() {
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
        let again = json!({"command": "cd /repo && cd sub && ls -la"});
        let pre = |input| PreToolUse {
            tool_name: "Bash",
            tool_input: input,
        };
        assert!(matches!(
            g.pre_tool(&pre(&again), &Ctx::new(&cx)),
            Some(PreToolDecision::Deny { .. })
        ));
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
