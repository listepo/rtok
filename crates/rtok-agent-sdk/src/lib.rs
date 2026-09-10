//! What every `rtok agent setup <host>` installer does, once.
//!
//! An agent host — Claude Code, Cursor, Codex, OpenCode, pi — is a config file rtok edits and,
//! for some of them, a plugin directory rtok links. The *shapes* differ (JSON hooks, a TOML
//! `[mcp_servers]` table, an `env` map, a symlink); the contract around them does not:
//!
//! - **Reversible.** Every file rtok touches is copied to `<name>.bak-<unix-seconds>` first, so
//!   one `.bak-*` per file is the whole undo.
//! - **Idempotent.** A second apply reports [`NO_CHANGES`] and writes nothing.
//! - **Dry-runnable.** `--dry-run` produces the same report and touches nothing at all.
//! - **Offered, never forced.** A host plugin is a prompt on a terminal, a no anywhere else
//!   (CI, a pipe, a host running setup for the user) unless `--yes` says otherwise.
//!
//! Every installer returns one report line per change. The report is also the write gate:
//! [`Apply`] refuses to write when the report is [`NO_CHANGES`], so "we said nothing changed"
//! and "we changed nothing" cannot drift apart.
//!
//! ```
//! use rtok_agent_sdk::{Apply, NO_CHANGES, register_mcp};
//! let dir = std::env::temp_dir().join(format!("rtok-agent-sdk-doc-{}", std::process::id()));
//! std::fs::create_dir_all(&dir).unwrap();
//! let path = dir.join("mcp.json");
//! let apply = Apply { dry_run: false, backup: true, yes: false };
//! let first = register_mcp(&apply, &path, "rtok", "rtok", &["mcp"]).unwrap();
//! assert_eq!(first, "mcpServers.rtok: rtok mcp");
//! assert_eq!(register_mcp(&apply, &path, "rtok", "rtok", &["mcp"]).unwrap(), NO_CHANGES);
//! std::fs::remove_dir_all(&dir).ok();
//! ```

use std::fs;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde_json::{Value, json};

/// The report an installer returns when it found nothing to do. Also the write gate: a run
/// carrying this report never touches the disk.
pub const NO_CHANGES: &str = "no changes";

/// How to install rtok when rtok itself is missing (D21 (5)). Named in every plugin offer,
/// because the offer is the one place a host's user reads before rtok exists for them.
pub const KETCH_INSTALL: &str = "ketch install listepo/rtok";

/// How much an installer is allowed to do to the disk on this run.
#[derive(Clone, Copy, Debug, Default)]
pub struct Apply {
    /// Describe the change and write nothing.
    pub dry_run: bool,
    /// Copy each file to `<name>.bak-<unix-seconds>` before the first write to it.
    pub backup: bool,
    /// Accept every offer without asking. The only way to say yes without a terminal.
    pub yes: bool,
}

impl Apply {
    /// True when this run may write a file for `report`.
    fn writes(&self, report: &str) -> bool {
        !self.dry_run && report != NO_CHANGES
    }
}

/// Copy `path` to `<name>.bak-<unix-seconds>` beside it. `None` when there is no file yet.
pub fn backup(path: &Path) -> Result<Option<PathBuf>> {
    if !path.exists() {
        return Ok(None);
    }
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    // Two commands inside one second would otherwise share a name and the first copy would go.
    let mut bak = path.with_file_name(format!("{name}.bak-{ts}"));
    for n in 1..100 {
        if !bak.exists() {
            break;
        }
        bak = path.with_file_name(format!("{name}.bak-{ts}-{n}"));
    }
    fs::copy(path, &bak).with_context(|| bak.display().to_string())?;
    Ok(Some(bak))
}

/// A host's JSON config as a value to edit. A missing or empty file is an empty object, not an
/// error: `agent setup` on a host the user has never configured is the common case.
pub fn read_json(path: &Path) -> Result<Value> {
    if !path.exists() {
        return Ok(json!({}));
    }
    let raw = fs::read_to_string(path).with_context(|| path.display().to_string())?;
    if raw.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(&raw).with_context(|| path.display().to_string())
}

/// Write `body` at `path`, gated by `apply` and `report`: a dry run and a [`NO_CHANGES`] report
/// write nothing, and the previous file is backed up first when asked.
///
/// Writes atomically: `body` lands in a sibling temp file first, then an `fs::rename` swaps it
/// over the target, so a kill/crash/full-disk between the two steps never leaves the host's
/// config empty or half-written — the target is either the old file or the fully-written new
/// one, never a partial write. When `path` is a symlink (dotfile managers replace host config
/// files with one), the temp file lands beside, and the rename lands on, the file it points to,
/// so the symlink itself survives.
pub fn write(apply: &Apply, path: &Path, body: &str, report: &str) -> Result<()> {
    if !apply.writes(report) {
        return Ok(());
    }
    if apply.backup {
        backup(path)?;
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).ok();
    }
    let target = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    write_atomic(&target, body).with_context(|| target.display().to_string())
}

/// [`write`]'s atomic swap: write `body` to `.<name>.rtok-tmp-<pid>` beside `target`, copy
/// `target`'s permissions onto it when `target` exists (a 0600 `~/.claude.json` must stay
/// 0600), then `fs::rename` the temp file over `target` — atomic on one filesystem. Any failed
/// step cleans up the temp file before returning the error.
fn write_atomic(target: &Path, body: &str) -> Result<()> {
    let dir = target.parent().unwrap_or(Path::new("."));
    let name = target.file_name().unwrap_or_default().to_string_lossy();
    let tmp = dir.join(format!(".{name}.rtok-tmp-{}", std::process::id()));
    let result: Result<()> = (|| -> Result<()> {
        fs::write(&tmp, body)?;
        if let Ok(meta) = fs::metadata(target) {
            fs::set_permissions(&tmp, meta.permissions())?;
        }
        fs::rename(&tmp, target)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// [`write`] for a JSON document: pretty-printed with the trailing newline host files carry.
pub fn write_json(apply: &Apply, path: &Path, root: &Value, report: &str) -> Result<()> {
    if !apply.writes(report) {
        return Ok(());
    }
    let body = serde_json::to_string_pretty(root)? + "\n";
    write(apply, path, &body, report)
}

/// Register a stdio MCP server under `mcpServers.<name>` in a host's JSON config — the shape
/// Claude Code (`~/.claude.json`) and Cursor (`~/.cursor/mcp.json`) both read.
pub fn register_mcp(
    apply: &Apply,
    path: &Path,
    name: &str,
    command: &str,
    args: &[&str],
) -> Result<String> {
    let mut root = read_json(path)?;
    if !root.is_object() {
        root = json!({});
    }
    let entry = json!({"type": "stdio", "command": command, "args": args});
    let servers = root
        .as_object_mut()
        .unwrap()
        .entry("mcpServers")
        .or_insert_with(|| json!({}));
    if !servers.is_object() {
        *servers = json!({});
    }
    if servers.get(name) == Some(&entry) {
        return Ok(NO_CHANGES.into());
    }
    servers[name] = entry;
    let report = format!("mcpServers.{name}: {command} {}", args.join(" "));
    write_json(apply, path, &root, &report)?;
    Ok(report)
}

/// Drop the `<name>` entry from a host's `mcpServers` map. Foreign servers are left alone, and a
/// map that ends up empty goes with it so the file reads as it did before rtok arrived.
pub fn unregister_mcp(apply: &Apply, path: &Path, name: &str) -> Result<String> {
    if !path.exists() {
        return Ok(NO_CHANGES.into());
    }
    let mut root = read_json(path)?;
    let Some(servers) = root.get_mut("mcpServers").and_then(Value::as_object_mut) else {
        return Ok(NO_CHANGES.into());
    };
    if servers.remove(name).is_none() {
        return Ok(NO_CHANGES.into());
    }
    if servers.is_empty() {
        root.as_object_mut().unwrap().remove("mcpServers");
    }
    let report = format!("- mcpServers.{name}");
    write_json(apply, path, &root, &report)?;
    Ok(report)
}

/// Ask, unless the answer is already known. `--yes` accepts without asking; on a terminal
/// dialoguer owns the prompt — it restores the terminal afterwards and reads Ctrl-C and EOF as a
/// no. Anywhere without a terminal an unanswered question is a no, so `agent setup` stays
/// non-interactive by default.
pub fn accepted(apply: &Apply, question: &str) -> bool {
    if apply.yes {
        return true;
    }
    if !std::io::stdin().is_terminal() {
        return false;
    }
    dialoguer::Confirm::new()
        .with_prompt(question)
        .default(true)
        .interact()
        .unwrap_or(false)
}

/// A host plugin directory this repo ships, and where that host loads it from (D21 (6)).
///
/// The link is a symlink on purpose: the host and the repo see the same tree, so an rtok update
/// is an update of the host's plugin with nothing to re-run.
pub struct PluginLink<'a> {
    /// Repo-relative source (`plugins/cursor`), named in every report so the user can find it.
    pub src_rel: &'a str,
    /// Absolute source directory in this repo.
    pub src: PathBuf,
    /// Where the host expects the plugin.
    pub dest: PathBuf,
    /// How the destination reads to a person (`~/.cursor/plugins/local`); a dry run prints
    /// the real path beside it, and every other line the label alone. `None` spells the
    /// destination path itself, with the source tree beside it on a dry run.
    pub label: Option<&'a str>,
    /// The host's name as its users spell it, for the question.
    pub host: &'a str,
}

impl PluginLink<'_> {
    /// True when something already sits at the destination — a link, a directory, anything.
    /// A foreign directory counts: rtok does not overwrite what it did not put there.
    pub fn linked(&self) -> bool {
        self.dest.symlink_metadata().is_ok()
    }

    /// The destination as the question and a declined offer spell it: the label, or the
    /// path itself when the host has no shorthand for where its plugins live.
    fn main_desc(&self) -> String {
        match self.label {
            Some(label) => label.to_string(),
            None => self.dest.display().to_string(),
        }
    }

    /// The destination as a dry run spells it: the label (or the path) with the concrete
    /// path beside it — the destination when a label stands in for it, the source tree the
    /// link will carry otherwise.
    fn dest_desc(&self) -> String {
        match self.label {
            Some(label) => format!("{label} ({})", self.dest.display()),
            None => format!("{} ({})", self.dest.display(), self.src.display()),
        }
    }

    /// Offer, link, or unlink. Returns the one-line report; a dry run and a declined offer both
    /// describe the offer and touch nothing.
    pub fn run(&self, apply: &Apply, remove: bool) -> Result<String> {
        if apply.dry_run {
            return Ok(format!(
                "offer {} → {} {KETCH_INSTALL}",
                self.src_rel,
                self.dest_desc()
            ));
        }
        if remove {
            if !self.linked() {
                return Ok(NO_CHANGES.into());
            }
            // Install refuses to overwrite a foreign directory; remove must not wipe one
            // either. Only unlink a symlink (or a plain file) that we could have created.
            let meta = self.dest.symlink_metadata()?;
            if meta.file_type().is_symlink() || meta.file_type().is_file() {
                fs::remove_file(&self.dest)?;
                return Ok(format!("- plugin {}", self.dest.display()));
            }
            return Ok(format!(
                "leave {} (not an rtok link; remove by hand)",
                self.dest.display()
            ));
        }
        if self.linked() {
            return Ok(NO_CHANGES.into());
        }
        let question = format!(
            "install {} into {} for {}?",
            self.src_rel,
            self.main_desc(),
            self.host
        );
        if !accepted(apply, &question) {
            return Ok(format!(
                "offer {} → {} (accept with --yes) {KETCH_INSTALL}",
                self.src_rel,
                self.main_desc()
            ));
        }
        if let Some(dir) = self.dest.parent() {
            fs::create_dir_all(dir).ok();
        }
        symlink(&self.src, &self.dest)?;
        let label = self.label.map(|l| format!(" {l}")).unwrap_or_default();
        Ok(format!(
            "+ plugin {} → {}{}",
            self.src_rel,
            self.dest.display(),
            label
        ))
    }
}

#[cfg(unix)]
fn symlink(src: &Path, dest: &Path) -> Result<()> {
    std::os::unix::fs::symlink(src, dest)
        .with_context(|| format!("symlink {} → {}", src.display(), dest.display()))
}

#[cfg(not(unix))]
fn symlink(src: &Path, dest: &Path) -> Result<()> {
    let _ = (src, dest);
    anyhow::bail!("plugin link requires unix")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("rtok-agent-sdk-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn apply() -> Apply {
        Apply {
            dry_run: false,
            backup: false,
            yes: false,
        }
    }

    /// The test harness has no terminal, which is exactly the CI / piped / host-driven case:
    /// the offer must decline itself rather than block waiting for an answer nobody can give.
    #[test]
    fn without_a_terminal_only_yes_accepts() {
        let mut a = apply();
        assert!(!accepted(&a, "install?"), "a headless run must not accept");
        a.yes = true;
        assert!(accepted(&a, "install?"), "--yes must accept without asking");
    }

    #[test]
    fn dry_run_writes_nothing_and_backup_keeps_the_old_file() {
        let dir = tmp("write");
        let path = dir.join("settings.json");
        let dry = Apply {
            dry_run: true,
            backup: true,
            yes: false,
        };
        write(&dry, &path, "{}", "+ something").unwrap();
        assert!(!path.exists(), "a dry run must not create the file");

        let mut a = apply();
        write(&a, &path, "one\n", "+ something").unwrap();
        write(&a, &path, "ignored\n", NO_CHANGES).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "one\n");

        a.backup = true;
        write(&a, &path, "two\n", "+ something else").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "two\n");
        let baks: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".bak-"))
            .collect();
        assert_eq!(baks.len(), 1, "one backup per write");
        assert_eq!(fs::read_to_string(baks[0].path()).unwrap(), "one\n");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn write_leaves_no_temp_file_behind() {
        let dir = tmp("atomic");
        let path = dir.join("settings.json");
        write(&apply(), &path, "one\n", "+ something").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "one\n");
        write(&apply(), &path, "two\n", "+ something else").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "two\n");
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".rtok-tmp-"))
            .collect();
        assert!(leftovers.is_empty(), "temp file left behind: {leftovers:?}");
        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn write_preserves_existing_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tmp("perms");
        let path = dir.join(".claude.json");
        fs::write(&path, "{}\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();

        write(&apply(), &path, "{\"a\":1}\n", "+ something").unwrap();

        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "write must not loosen an existing file's permissions"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn write_through_a_symlink_updates_the_target_and_keeps_the_link() {
        let dir = tmp("symlink");
        let real = dir.join("real.json");
        fs::write(&real, "{}\n").unwrap();
        let link = dir.join("linked.json");
        symlink(&real, &link).unwrap();

        write(&apply(), &link, "{\"a\":1}\n", "+ something").unwrap();

        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink(),
            "write must not replace the symlink with a plain file"
        );
        assert_eq!(fs::read_to_string(&real).unwrap(), "{\"a\":1}\n");
        assert_eq!(fs::read_to_string(&link).unwrap(), "{\"a\":1}\n");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn mcp_registration_is_idempotent_and_leaves_foreign_servers() {
        let dir = tmp("mcp");
        let path = dir.join("mcp.json");
        fs::write(&path, r#"{"mcpServers":{"other":{"command":"x"}}}"#).unwrap();
        let a = apply();
        assert_eq!(
            register_mcp(&a, &path, "rtok", "rtok", &["mcp"]).unwrap(),
            "mcpServers.rtok: rtok mcp"
        );
        assert_eq!(
            register_mcp(&a, &path, "rtok", "rtok", &["mcp"]).unwrap(),
            NO_CHANGES
        );
        assert_eq!(
            unregister_mcp(&a, &path, "rtok").unwrap(),
            "- mcpServers.rtok"
        );
        assert_eq!(unregister_mcp(&a, &path, "rtok").unwrap(), NO_CHANGES);
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("other"), "{raw}");
        assert!(!raw.contains("rtok"), "{raw}");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn plugin_link_offers_then_links_then_unlinks() {
        let dir = tmp("link");
        let src = dir.join("plugins/demo");
        fs::create_dir_all(&src).unwrap();
        let link = PluginLink {
            src_rel: "plugins/demo",
            src,
            dest: dir.join("host/plugins/rtok"),
            label: Some("~/.demo/plugins"),
            host: "demo",
        };

        let dry = link
            .run(
                &Apply {
                    dry_run: true,
                    backup: false,
                    yes: false,
                },
                false,
            )
            .unwrap();
        assert!(
            dry.starts_with(&format!(
                "offer plugins/demo → ~/.demo/plugins ({}) {KETCH_INSTALL}",
                link.dest.display()
            )),
            "{dry}"
        );
        assert!(!link.linked(), "a dry run must not link");

        // A host with no shorthand for its plugin dir (pi) spells the destination path
        // itself, with the source tree beside it on a dry run.
        let bare = PluginLink {
            src_rel: "plugins/demo",
            src: link.src.clone(),
            dest: dir.join("other/plugins/rtok"),
            label: None,
            host: "demo",
        };
        let bare_dry = bare
            .run(
                &Apply {
                    dry_run: true,
                    backup: false,
                    yes: false,
                },
                false,
            )
            .unwrap();
        assert_eq!(
            bare_dry,
            format!(
                "offer plugins/demo → {} ({}) {KETCH_INSTALL}",
                bare.dest.display(),
                link.src.display()
            )
        );

        // No terminal, no --yes: the offer declines itself and still says how to accept.
        let declined = link.run(&apply(), false).unwrap();
        assert_eq!(
            declined,
            format!("offer plugins/demo → ~/.demo/plugins (accept with --yes) {KETCH_INSTALL}")
        );
        assert!(!link.linked());

        let yes = Apply {
            dry_run: false,
            backup: false,
            yes: true,
        };
        assert_eq!(
            link.run(&yes, false).unwrap(),
            format!(
                "+ plugin plugins/demo → {} ~/.demo/plugins",
                link.dest.display()
            )
        );
        assert!(link.linked());
        assert_eq!(link.run(&yes, false).unwrap(), NO_CHANGES);
        assert!(link.run(&yes, true).unwrap().starts_with("- plugin"));
        assert!(!link.linked());
        assert_eq!(link.run(&yes, true).unwrap(), NO_CHANGES);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn remove_leaves_a_foreign_directory_alone() {
        let dir = tmp("foreign");
        let dest = dir.join("host/plugins/rtok");
        fs::create_dir_all(&dest).unwrap();
        fs::write(dest.join("mine.txt"), "keep").unwrap();
        let link = PluginLink {
            src_rel: "plugins/demo",
            src: dir.join("plugins/demo"),
            dest: dest.clone(),
            label: Some("~/.demo/plugins"),
            host: "demo",
        };
        let yes = Apply {
            dry_run: false,
            backup: false,
            yes: true,
        };
        let report = link.run(&yes, true).unwrap();
        assert!(
            report.starts_with("leave "),
            "foreign dir must survive: {report}"
        );
        assert!(dest.join("mine.txt").exists(), "contents must stay");
        let _ = fs::remove_dir_all(dir);
    }
}
