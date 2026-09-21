//! T78: the installers against copies of this machine's own agent configs.
//!
//! Every other agents test writes a synthetic config — a couple of keys, all of them ours.
//! That never meets what install actually edits: a 25 KB `settings.json`, a `hooks.json`
//! already carrying other tools' hooks, an `mcp.json` holding a dozen foreign servers. A
//! regression that dropped or rewrote foreign content would pass the whole suite.
//!
//! Each test copies the user's real file into a throwaway `HOME` (the original is never
//! opened for writing), runs `agents install <host> --yes` and then `agents remove <host>`,
//! and checks that every entry rtok does not own is still there, unchanged, after both — and
//! that a second install changes nothing a first one did not.
//!
//! Local-only by construction: [`common::agents::real_config`] returns `None` when the file
//! is absent or `CI` is set, and the test prints a skip line instead of failing. Nothing here
//! asserts that any particular host is installed on the machine running it.

mod common;

use common::agents::{raw, seed_real, skip, tmp, write_cfg};
use serde_json::Value;
use std::fs;
use std::path::Path;

/// A host and the home-relative configs it writes, spelled as they sit under a real `$HOME`.
struct Host {
    id: &'static str,
    files: &'static [&'static str],
}

const HOSTS: &[Host] = &[
    Host {
        id: "cursor",
        files: &[".cursor/hooks.json", ".cursor/mcp.json"],
    },
    Host {
        id: "claude",
        files: &[".claude/settings.json"],
    },
    Host {
        id: "codex",
        files: &[".codex/config.toml"],
    },
    Host {
        id: "opencode",
        files: &[".config/opencode/opencode.json"],
    },
    Host {
        id: "kimi",
        files: &[".kimi-code/config.toml"],
    },
    Host {
        id: "zed",
        files: &[".config/zed/settings.json"],
    },
    Host {
        id: "windsurf",
        files: &[".codeium/windsurf/mcp_config.json"],
    },
    Host {
        id: "zcode",
        files: &[".zcode/cli/config.json"],
    },
    Host {
        id: "vscode",
        files: &["Library/Application Support/Code/User/settings.json"],
    },
];

/// True for anything rtok owns: install puts it there and remove takes it away, so it is the
/// one thing this test must not hold on to. Matched on the serialized value, which catches a
/// server entry, a hook command and a proxy URL alike.
fn ours(v: &Value) -> bool {
    v.to_string().to_ascii_lowercase().contains("rtok")
}

/// Every foreign leaf `before` carried, still in `after` under the same path and with the same
/// value. Object members are matched by key; array elements by containment, because rtok
/// appends its own hook entries and the indices of foreign ones shift under them.
fn json_survives(
    before: &Value,
    after: &Value,
    at: &str,
    checked: &mut usize,
    lost: &mut Vec<String>,
) {
    match (before, after) {
        (Value::Object(b), Value::Object(a)) => {
            for (k, v) in b {
                if k.to_ascii_lowercase().contains("rtok") || ours(v) {
                    continue;
                }
                match a.get(k) {
                    Some(av) => json_survives(v, av, &format!("{at}/{k}"), checked, lost),
                    None => {
                        *checked += 1;
                        lost.push(format!("{at}/{k}"));
                    }
                }
            }
        }
        (Value::Array(b), Value::Array(a)) => {
            for (i, v) in b.iter().enumerate() {
                if ours(v) {
                    continue;
                }
                *checked += 1;
                if !a.contains(v) {
                    lost.push(format!("{at}[{i}]"));
                }
            }
        }
        _ => {
            *checked += 1;
            if before != after {
                lost.push(at.to_string());
            }
        }
    }
}

/// The same claim for a file that is not JSON (TOML, or JSON with comments): every foreign
/// line still present, indentation ignored. Weaker than the structural check and uniform
/// across formats, which is what a fallback has to be.
fn lines_survive(before: &str, after: &str, checked: &mut usize, lost: &mut Vec<String>) {
    let kept: Vec<&str> = after.lines().map(str::trim).collect();
    for line in before.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') || line.to_ascii_lowercase().contains("rtok") {
            continue;
        }
        *checked += 1;
        if !kept.contains(&line) {
            lost.push(line.to_string());
        }
    }
}

/// `before` must survive into `after`; returns how many entries were actually compared, so a
/// caller can refuse a vacuous pass.
fn survives(before: &str, after: &str, stage: &str, path: &Path) -> usize {
    let (mut checked, mut lost) = (0usize, Vec::new());
    match (
        serde_json::from_str::<Value>(before),
        serde_json::from_str::<Value>(after),
    ) {
        (Ok(b), Ok(a)) => json_survives(&b, &a, "", &mut checked, &mut lost),
        (Ok(_), Err(e)) => panic!(
            "{} is no longer valid JSON after {stage}: {e}",
            path.display()
        ),
        _ => lines_survive(before, after, &mut checked, &mut lost),
    }
    assert!(
        lost.is_empty(),
        "{stage} dropped or rewrote {} foreign entries in {}: {lost:#?}",
        lost.len(),
        path.display()
    );
    checked
}

/// Seed one host's real configs, install, install again, remove — and hold every foreign entry
/// to its seeded value at each stage.
fn round_trip(host: &Host) {
    let home = tmp(&format!("real-{}", host.id));
    let cfg = write_cfg(&home);
    let seeded: Vec<_> = host
        .files
        .iter()
        .filter_map(|rel| {
            seed_real(&home, rel).map(|dest| (*rel, fs::read_to_string(&dest).unwrap(), dest))
        })
        .collect();
    if seeded.is_empty() {
        skip(&format!("{} against a real config", host.id));
        let _ = fs::remove_dir_all(&home);
        return;
    }

    let install = ["agents", "install", host.id, "--yes"];
    let out = raw(&install, &cfg, &home);
    assert!(
        out.status.success(),
        "install {} failed: {}",
        host.id,
        String::from_utf8_lossy(&out.stderr)
    );

    let mut compared = 0;
    let after_install: Vec<String> = seeded
        .iter()
        .map(|(_, before, dest)| {
            let after = fs::read_to_string(dest).unwrap();
            compared += survives(before, &after, "install", dest);
            after
        })
        .collect();
    assert!(
        compared > 0,
        "{}: nothing foreign in the seeded configs, so this proves nothing",
        host.id
    );

    // Idempotent on a real file, not just on one we wrote: a second install leaves the exact
    // bytes the first one produced.
    assert!(raw(&install, &cfg, &home).status.success());
    for ((rel, _, dest), once) in seeded.iter().zip(&after_install) {
        assert_eq!(
            &fs::read_to_string(dest).unwrap(),
            once,
            "{}: second install rewrote {rel}",
            host.id
        );
    }

    assert!(
        raw(&["agents", "remove", host.id], &cfg, &home)
            .status
            .success()
    );
    for (_, before, dest) in &seeded {
        survives(before, &fs::read_to_string(dest).unwrap(), "remove", dest);
    }
    let _ = fs::remove_dir_all(&home);
}

fn by_id(id: &str) -> &'static Host {
    HOSTS.iter().find(|h| h.id == id).expect("host in HOSTS")
}

macro_rules! real_config_round_trip {
    ($($name:ident => $id:literal),* $(,)?) => {
        $(#[test] fn $name() { round_trip(by_id($id)); })*
    };
}

real_config_round_trip! {
    cursor_keeps_the_real_hooks_and_mcp_json => "cursor",
    claude_keeps_the_real_settings_json => "claude",
    codex_keeps_the_real_config_toml => "codex",
    opencode_keeps_the_real_opencode_json => "opencode",
    kimi_keeps_the_real_config_toml => "kimi",
    windsurf_keeps_the_real_mcp_config_json => "windsurf",
    zcode_keeps_the_real_config_json => "zcode",
    // VS Code's settings.json may be JSONC too; whether this machine's is decides whether this
    // passes or reproduces T79, which is the honest answer for a test that reads a real file.
    vscode_keeps_the_real_settings_json => "vscode",
}

/// Zed writes **JSONC** — its `settings.json` carries `//` comments and trailing commas, which
/// `rtok_agent_sdk::read_json` (strict `serde_json`) refuses, so `agents install zed` aborts
/// against a file Zed itself wrote. Tracked as T79; un-ignore as that task's check. Ignored,
/// not deleted: the reproduction is the point.
#[test]
#[ignore = "T79: a real Zed settings.json is JSONC and the installer cannot read it"]
fn zed_keeps_the_real_settings_json() {
    round_trip(by_id("zed"));
}

/// The gate itself, both sides: `CI` hides every host config, including the ones that really
/// are on this machine, and without `CI` the answer is exactly "does the file exist".
///
/// Without this the suite could not tell a working gate from one that had quietly stopped
/// finding anything — the visible result is the same skip line either way.
#[test]
fn ci_hides_what_this_machine_really_has() {
    use common::agents::{real_config_from, real_home};
    let home = real_home();
    let mut found = 0;
    for rel in HOSTS.iter().flat_map(|h| h.files) {
        assert!(
            real_config_from(true, home.as_deref(), rel).is_none(),
            "CI must hide {rel}"
        );
        let off = real_config_from(false, home.as_deref(), rel);
        assert_eq!(
            off.is_some(),
            home.as_deref().is_some_and(|h| h.join(rel).is_file()),
            "off CI, {rel} must answer exactly whether the file exists"
        );
        found += usize::from(off.is_some());
    }
    if found == 0 {
        skip("the whole real-config suite: this machine runs none of these hosts");
    }
}
