//! `apply`: the one entry point a host installer calls to install, update or remove rtok's MCP
//! entry (T277) — instead of each host writing its own `register_mcp` / `unregister_mcp`.

use anyhow::Result;
use serde_json::Value;

use crate::config::{self, Fs, Written};
use crate::spec::McpSpec;

/// What `apply` should do to the entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Install,
    Update,
    Remove,
}

/// What `apply` did, or refused to do and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Report {
    NoChanges,
    Wrote(String),
    /// Left alone: the entry is not rtok's, or a user edited it since rtok wrote it. PR 1 has
    /// no host wired up yet, so there is no "remove anyway?" prompt behind this — that stays
    /// `rtok_agent_sdk`'s `judge_owned`/`keep_edited` job (interactive, real files) until a
    /// later PR gives `Mode::Remove` the same `force` a host's CLI flag would set.
    Left(String),
}

/// Install, update, or remove `spec`'s entry. Never silently removes an entry the user edited
/// or that never was rtok's (T246's ownership rule, restated behind this crate's own [`Fs`]).
pub fn apply(spec: &McpSpec, mode: Mode, fs: &mut impl Fs) -> Result<Report> {
    let ours = spec.entry();
    let at = format!("{}.{}", spec.key_path.join("."), spec.server.name);
    match mode {
        Mode::Install | Mode::Update => match config::write_entry(fs, spec, &ours)? {
            Written::Unchanged => Ok(Report::NoChanges),
            Written::Changed => Ok(Report::Wrote(at)),
        },
        Mode::Remove => {
            let Some(have) = config::read_entry(fs, spec)? else {
                return Ok(Report::NoChanges);
            };
            if let Some(reason) = not_removable(&have, &ours) {
                return Ok(Report::Left(format!("leave {at} ({reason})")));
            }
            match config::remove_entry(fs, spec)? {
                Written::Unchanged => Ok(Report::NoChanges),
                Written::Changed => Ok(Report::Wrote(format!("- {at}"))),
            }
        }
    }
}

/// `None` when `have` is safe to remove; `Some(reason)` otherwise — mirrors
/// `rtok_agent_sdk::judge_owned`'s two checks (not rtok's at all; rtok's but user-edited) at a
/// smaller scope, since no host calls this yet and there is nowhere to ask "remove anyway?".
fn not_removable(have: &Value, ours: &Value) -> Option<&'static str> {
    if !runs_rtok(have) {
        return Some("not rtok's; remove by hand");
    }
    (have != ours).then_some("changed by you; remove by hand")
}

/// True when `v`, or anything nested in it, names the rtok binary — bare or as a path.
fn runs_rtok(v: &Value) -> bool {
    match v {
        Value::String(s) => s == "rtok" || s.ends_with("/rtok") || s.ends_with("\\rtok.exe"),
        Value::Array(a) => a.iter().any(runs_rtok),
        Value::Object(m) => m.values().any(runs_rtok),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::registry;
    use crate::spec::{DuplicateName, EntryShape, Format, Surface};
    use crate::status::{self, Entry};

    /// An in-memory [`Fs`] for tests: no host disk touched, `backups` counts every call so a
    /// test can assert `apply` backs up before it writes (T277 PR1; the real `Vfs`-backed impl
    /// lands with the first host in PR2).
    #[derive(Default)]
    struct MemFs {
        files: BTreeMap<PathBuf, Vec<u8>>,
        backups: usize,
    }

    impl Fs for MemFs {
        fn read(&self, path: &Path) -> Option<Vec<u8>> {
            self.files.get(path).cloned()
        }
        fn write(&mut self, path: &Path, bytes: Vec<u8>) -> Result<()> {
            self.files.insert(path.to_path_buf(), bytes);
            Ok(())
        }
        fn backup(&mut self, _path: &Path) -> Result<()> {
            self.backups += 1;
            Ok(())
        }
    }

    fn spec(format: Format, key_path: &str) -> McpSpec {
        McpSpec {
            host: "test",
            surface: Surface::Cli,
            config_path: PathBuf::from("/config"),
            format,
            key_path: McpSpec::key_path(key_path),
            server: registry::RTOK,
            entry_shape: EntryShape::TypedCommand,
            duplicate_name: DuplicateName::ShowsBoth,
            plugin_serves: None,
        }
    }

    /// One row of the table: a format, its dotted key path (ZCode's `mcp.servers` is the two
    /// -level case), and whatever the file holds before rtok ever touches it.
    struct Case {
        format: Format,
        key_path: &'static str,
        existing: &'static str,
    }

    // Jsonc and Toml rows join this table in the crate's second commit, once `config` grows
    // their format support; the dotted-key case (ZCode's real `mcp.servers` shape is Toml) is
    // covered here against Json first, since the dotted-path walk itself is format-agnostic.
    const CASES: &[Case] = &[
        Case {
            format: Format::Json,
            key_path: "mcpServers",
            existing: "",
        },
        Case {
            format: Format::Json,
            key_path: "mcp.servers",
            existing: "",
        },
    ];

    #[test]
    fn install_then_remove_round_trips_across_every_format() {
        for case in CASES {
            let spec = spec(case.format, case.key_path);
            let mut fs = MemFs::default();
            if !case.existing.is_empty() {
                fs.write(&spec.config_path, case.existing.as_bytes().to_vec())
                    .unwrap();
            }

            let install = apply(&spec, Mode::Install, &mut fs).unwrap();
            assert!(
                matches!(install, Report::Wrote(_)),
                "{:?}: {install:?}",
                case.format
            );
            assert_eq!(
                status::status(&fs, &spec).unwrap().entry,
                Entry::Present,
                "{:?}",
                case.format
            );
            assert_eq!(
                fs.backups, 1,
                "{:?}: install must back up first",
                case.format
            );

            // A second install of the same entry is a no-op — the shared "no changes" gate.
            assert_eq!(
                apply(&spec, Mode::Install, &mut fs).unwrap(),
                Report::NoChanges
            );
            assert_eq!(
                fs.backups, 1,
                "{:?}: a no-op write must not back up again",
                case.format
            );

            if !case.existing.is_empty() {
                let after = String::from_utf8(fs.read(&spec.config_path).unwrap()).unwrap();
                assert!(
                    after.contains("// keep me"),
                    "{:?}: lost a comment: {after}",
                    case.format
                );
                assert!(
                    after.contains("\"other\": 1"),
                    "{:?}: lost a key: {after}",
                    case.format
                );
            }

            let remove = apply(&spec, Mode::Remove, &mut fs).unwrap();
            assert!(
                matches!(remove, Report::Wrote(_)),
                "{:?}: {remove:?}",
                case.format
            );
            assert_eq!(
                config::read_entry(&fs, &spec).unwrap(),
                None,
                "{:?}",
                case.format
            );
            assert_eq!(
                apply(&spec, Mode::Remove, &mut fs).unwrap(),
                Report::NoChanges
            );
        }
    }

    #[test]
    fn remove_leaves_an_entry_the_user_edited() {
        let spec = spec(Format::Json, "mcpServers");
        let mut fs = MemFs::default();
        apply(&spec, Mode::Install, &mut fs).unwrap();
        let edited = br#"{"mcpServers":{"rtok":{"type":"stdio","command":"rtok","args":["mcp","--extra"]}}}"#;
        fs.write(&spec.config_path, edited.to_vec()).unwrap();

        let report = apply(&spec, Mode::Remove, &mut fs).unwrap();
        assert!(matches!(report, Report::Left(_)), "{report:?}");
        assert!(config::read_entry(&fs, &spec).unwrap().is_some());
    }

    #[test]
    fn remove_leaves_an_entry_that_is_not_rtoks() {
        let spec = spec(Format::Json, "mcpServers");
        let mut fs = MemFs::default();
        fs.write(
            &spec.config_path,
            br#"{"mcpServers":{"rtok":{"command":"other-tool"}}}"#.to_vec(),
        )
        .unwrap();

        let report = apply(&spec, Mode::Remove, &mut fs).unwrap();
        assert!(matches!(report, Report::Left(_)), "{report:?}");
        assert!(config::read_entry(&fs, &spec).unwrap().is_some());
    }
}
