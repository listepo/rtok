//! One version scheme for a plugin across every install source — GitHub, local checkout and
//! marketplace catalog (plan T279). The version-file format, the per-host install receipt,
//! the installed/available version lookup and the update/skip/reinstall decision, plus (PR 3)
//! `git describe` for a local build and parsing `--source` off the CLI. `agents update` wires
//! this in for Claude (the worked example the docs task names); other hosts still reinstall
//! unconditionally until they grow the same plumbing.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;

use anyhow::{Context, Result, bail};
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::config::Config;

/// `.rtok-plugin-version`'s only schema so far (plan T279 step 1). A file with a different
/// `schema` is refused rather than misread.
const SCHEMA: u32 = 1;

/// Where a plugin came from — carried by [`VersionFile`] (local builds only), [`ReceiptEntry`]
/// and [`Available`], and compared by [`decide`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Github,
    Local,
    Marketplace,
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Source::Github => "github",
            Source::Local => "local",
            Source::Marketplace => "marketplace",
        })
    }
}

impl FromStr for Source {
    type Err = anyhow::Error;

    /// `agents update --source github|local|marketplace` (T279 PR 3). Case-insensitive so a
    /// shell-completed or hand-typed `--source GitHub` still parses.
    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "github" => Ok(Source::Github),
            "local" => Ok(Source::Local),
            "marketplace" => Ok(Source::Marketplace),
            other => {
                bail!("unknown plugin source {other:?} (expected github, local or marketplace)")
            }
        }
    }
}

/// `plugins/<host>/.rtok-plugin-version`: committed at the plugin root, copied with the
/// plugin by every install source. `source` is only set by a local install (T279 step 1); a
/// committed file carries none.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VersionFile {
    pub schema: u32,
    pub plugin: String,
    pub version: Version,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
}

impl VersionFile {
    /// Exercised by this module's own round-trip tests; writing a fresh `.rtok-plugin-version`
    /// into an installed copy is a later task (resolving that path needs the version-numbered
    /// cache directory a host names only after install, out of PR 3's scope).
    #[allow(dead_code)]
    pub fn new(plugin: impl Into<String>, version: Version) -> Self {
        Self {
            schema: SCHEMA,
            plugin: plugin.into(),
            version,
            source: None,
        }
    }

    /// Reads and validates a `.rtok-plugin-version` file. An unknown `schema` or a `version`
    /// that does not parse as SemVer is an error naming `path`.
    pub fn read(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let file: Self = serde_json::from_str(&text)
            .with_context(|| format!("{}: invalid plugin version file", path.display()))?;
        if file.schema != SCHEMA {
            bail!(
                "{}: unknown schema {} (rtok understands schema {SCHEMA})",
                path.display(),
                file.schema
            );
        }
        Ok(file)
    }

    /// Writes the compact, single-line JSON shape the committed files use, LF-terminated.
    /// Same PR 3 scope note as [`VersionFile::new`].
    #[allow(dead_code)]
    pub fn write(&self, path: &Path) -> Result<()> {
        let body = format!("{}\n", serde_json::to_string(self)?);
        crate::config::write_file(path, &body)
    }
}

/// One host's row in the [`Receipt`] (plan T279 step 1). `reference` is the tag/branch for a
/// GitHub install or the checkout path for a local one; `marketplace` is set only for a
/// marketplace install.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReceiptEntry {
    pub source: Source,
    #[serde(rename = "ref")]
    pub reference: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marketplace: Option<String>,
    pub path: PathBuf,
    pub version: Version,
    pub installed_at: String,
}

/// The install receipt, one row per host, at [`receipt_path`]. Missing on disk reads as empty
/// rather than an error — nothing has ever installed through rtok yet.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Receipt(BTreeMap<String, ReceiptEntry>);

impl Receipt {
    pub fn read(path: &Path) -> Result<Self> {
        match fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text)
                .with_context(|| format!("{}: invalid plugin receipt", path.display())),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).context(format!("read {}", path.display())),
        }
    }

    pub fn write(&self, path: &Path) -> Result<()> {
        let body = format!("{}\n", serde_json::to_string_pretty(&self.0)?);
        crate::config::write_file(path, &body)
    }

    pub fn get(&self, host: &str) -> Option<&ReceiptEntry> {
        self.0.get(host)
    }

    pub fn upsert(&mut self, host: impl Into<String>, entry: ReceiptEntry) {
        self.0.insert(host.into(), entry);
    }

    /// A reinstall failure deleting the receipt (plan T279 step 6, `--force`'s own path) is
    /// out of PR 3's scope: this PR leaves the receipt untouched on any `claude` failure, so
    /// the next `agents update` sees the same state and tries again.
    #[allow(dead_code)]
    pub fn delete(&mut self, host: &str) -> Option<ReceiptEntry> {
        self.0.remove(host)
    }
}

/// `$XDG_STATE_HOME/rtok/plugins.json` and its OS equivalents (plan T279 step 1), given an
/// already-resolved home and the two env vars that redirect it — pure so the OS branches are
/// unit-tested without touching the real environment or disk.
fn default_receipt_path(
    home: &Path,
    xdg_state_home: Option<&str>,
    local_appdata: Option<&str>,
) -> PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/rtok/plugins.json")
    } else if cfg!(target_os = "windows") {
        local_appdata
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join("AppData/Local"))
            .join("rtok/plugins.json")
    } else {
        xdg_state_home
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/state"))
            .join("rtok/plugins.json")
    }
}

/// The receipt path for this machine: `cfg.plugin_receipt_path` if a caller (or a test) set
/// one, else the OS default (D29 — production never sets the override).
pub fn receipt_path(cfg: &Config) -> PathBuf {
    cfg.plugin_receipt_path.clone().unwrap_or_else(|| {
        default_receipt_path(
            &super::home_dir(),
            std::env::var("XDG_STATE_HOME").ok().as_deref(),
            std::env::var("LOCALAPPDATA").ok().as_deref(),
        )
    })
}

/// Build metadata for a local install: `+g<sha>` clean, `+g<sha>.dirty` uncommitted, from a
/// `git describe --always --dirty`-style string — a bare abbreviated sha with no tags, or
/// `<tag>-<n>-g<sha>` with them, optionally `-dirty` suffixed. Pure: no git call here.
pub fn local_version(base: &Version, describe: &str) -> Version {
    let (rest, dirty) = describe
        .strip_suffix("-dirty")
        .map_or((describe, false), |r| (r, true));
    let sha = rest.rsplit_once("-g").map_or(rest, |(_, sha)| sha);
    let build = if dirty {
        format!("g{sha}.dirty")
    } else {
        format!("g{sha}")
    };
    let mut version = base.clone();
    version.build =
        semver::BuildMetadata::new(&build).expect("git sha + '.dirty' is valid build metadata");
    version
}

/// A resolved installed plugin: version, source and whether it predates T279 (no version file
/// was found — see [`installed`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Installed {
    pub version: Version,
    pub source: Source,
    pub legacy: bool,
}

/// The installed-version lookup (plan T279 step 2/4): the `.rtok-plugin-version` inside the
/// installed copy first, then the receipt, then a host record's own version string (for
/// Claude, `installed_plugins.json`'s `version`), else `0.0.0`. `legacy` is true whenever no
/// version file was found — the state every install was in before this task. Pure over
/// already-read inputs; [`read_installed`] is the thin reader that gets them from disk.
pub fn installed(
    version_file: Option<&VersionFile>,
    receipt_entry: Option<&ReceiptEntry>,
    host_record_version: Option<&str>,
    fallback_source: Source,
) -> Installed {
    let legacy = version_file.is_none();
    let version = version_file
        .map(|f| f.version.clone())
        .or_else(|| receipt_entry.map(|e| e.version.clone()))
        .or_else(|| host_record_version.and_then(|v| Version::parse(v).ok()))
        .unwrap_or_else(|| Version::new(0, 0, 0));
    let source = version_file
        .and_then(|f| f.source)
        .or_else(|| receipt_entry.map(|e| e.source))
        .unwrap_or(fallback_source);
    Installed {
        version,
        source,
        legacy,
    }
}

/// Reads the version file (if the installed copy has one) and the receipt row, then applies
/// [`installed`] — the thin I/O wrapper the pure lookup is built on. Not called yet: Claude's
/// `plugin_update` (T279 PR 3) only has the receipt and the host record — locating the
/// installed copy's own file needs the version-numbered cache directory a host names only
/// after install, a later task.
#[allow(dead_code)]
pub fn read_installed(
    version_file_path: &Path,
    receipt: &Receipt,
    host: &str,
    host_record_version: Option<&str>,
    fallback_source: Source,
) -> Result<Installed> {
    let version_file = if version_file_path.is_file() {
        Some(VersionFile::read(version_file_path)?)
    } else {
        None
    };
    Ok(installed(
        version_file.as_ref(),
        receipt.get(host),
        host_record_version,
        fallback_source,
    ))
}

/// The new version available from a source, e.g. read from
/// `plugins/<host>/.rtok-plugin-version` at the matching tag (T279 step 2).
#[derive(Debug, Clone, PartialEq)]
pub struct Available {
    pub version: Version,
    pub source: Source,
}

/// The version available from `source` (plan T279 step 2), given the plugin's local tree
/// (`plugins/<host>` for a local install; unused otherwise) and the running rtok's own
/// version. GitHub and marketplace both read as `running_version`: PR 1's `--check` (and its
/// Rust twin) keep every `.rtok-plugin-version` and manifest equal to `CARGO_PKG_VERSION`, so
/// the tag or catalog entry matching this binary is already known — no network call, and
/// `agents outdated` (T279.1) works offline. Only a local checkout can genuinely differ from
/// what is running, so it alone reads its own file and `git describe`.
pub fn available(source: Source, local_dir: &Path, running_version: &Version) -> Result<Available> {
    if source != Source::Local {
        return Ok(Available {
            version: running_version.clone(),
            source,
        });
    }
    let file = VersionFile::read(&local_dir.join(".rtok-plugin-version"))?;
    let describe = git_describe(local_dir)?;
    Ok(Available {
        version: local_version(&file.version, &describe),
        source,
    })
}

/// `git -C <dir> describe --always --dirty`, trimmed — the only place this module spawns a
/// process.
fn git_describe(dir: &Path) -> Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["describe", "--always", "--dirty"])
        .output()
        .with_context(|| format!("cannot run git describe in {}", dir.display()))?;
    if !out.status.success() {
        bail!(
            "git describe in {}: {}",
            dir.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// What `agents update` should do for one host (plan T279 step 3/4/6): the pure decision, no
/// files, no processes.
#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    /// Already at `available`'s version and build metadata — nothing to do.
    Skip { reason: String },
    /// `available` is newer, or the same base version with different build metadata (a new
    /// local build): update in place.
    Update,
    /// The recorded source changed, or `--force`: remove and install again.
    Reinstall { reason: String },
    /// No installed row: install fresh.
    Install,
    /// `available` is older than what is installed: leave it, warn.
    SkipOlder { warning: String },
}

/// The pure decision. `force` bypasses every other rule (reinstall when installed, install
/// when not — the comparison below never runs). Otherwise: a source change always reinstalls;
/// else compare by SemVer *precedence* (`Version::cmp_precedence`, which disregards build
/// metadata per the SemVer spec — plain `Version::cmp`/`Ord` does not: it orders build
/// metadata lexicographically as a last tiebreaker, which would call two builds of the same
/// base version "older"/"newer" instead of just differently built) — newer updates, older
/// skips with a warning, equal precedence with equal build metadata skips, equal precedence
/// with different build metadata updates (a local rebuild at the same base version).
pub fn decide(installed: Option<Installed>, available: &Available, force: bool) -> Decision {
    let Some(installed) = installed else {
        return Decision::Install;
    };
    if force {
        return Decision::Reinstall {
            reason: "--force".to_string(),
        };
    }
    if installed.source != available.source {
        return Decision::Reinstall {
            reason: format!(
                "source changed from {} to {}",
                installed.source, available.source
            ),
        };
    }
    match available.version.cmp_precedence(&installed.version) {
        Ordering::Greater => Decision::Update,
        Ordering::Less => Decision::SkipOlder {
            warning: format!(
                "available {} is older than installed {}",
                available.version, installed.version
            ),
        },
        Ordering::Equal if available.version.build == installed.version.build => Decision::Skip {
            reason: format!("{} up to date", installed.version),
        },
        Ordering::Equal => Decision::Update,
    }
}

/// `installed_at` for a fresh [`ReceiptEntry`]: RFC 3339 UTC, second precision, from the same
/// clock and calendar `rtok_log::stamp` uses for log lines.
pub fn now_iso() -> String {
    format!(
        "{}Z",
        rtok_log::stamp(rtok_log::now()).replacen(' ', "T", 1)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    fn available(version: &str, source: Source) -> Available {
        Available {
            version: v(version),
            source,
        }
    }

    fn installed_at(version: &str, source: Source, legacy: bool) -> Installed {
        Installed {
            version: v(version),
            source,
            legacy,
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("rtok-plugin-version-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    // ── decide ──────────────────────────────────────────────────────────────

    #[test]
    fn equal_version_and_build_skips() {
        let i = installed_at("0.10.0", Source::Github, false);
        let a = available("0.10.0", Source::Github);
        assert!(matches!(decide(Some(i), &a, false), Decision::Skip { .. }));
    }

    #[test]
    fn newer_available_updates() {
        let i = installed_at("0.10.0", Source::Github, false);
        let a = available("0.11.0", Source::Github);
        assert_eq!(decide(Some(i), &a, false), Decision::Update);
    }

    #[test]
    fn older_available_skips_with_a_warning_naming_both() {
        let i = installed_at("0.10.0", Source::Github, false);
        let a = available("0.9.0", Source::Github);
        match decide(Some(i), &a, false) {
            Decision::SkipOlder { warning } => {
                assert!(warning.contains("0.9.0"), "{warning}");
                assert!(warning.contains("0.10.0"), "{warning}");
            }
            other => panic!("expected SkipOlder, got {other:?}"),
        }
    }

    #[test]
    fn same_base_different_build_metadata_updates() {
        let i = installed_at("0.10.0+g111aaaa", Source::Local, false);
        let a = available("0.10.0+g222bbbb", Source::Local);
        assert_eq!(decide(Some(i), &a, false), Decision::Update);
    }

    #[test]
    fn dirty_against_clean_at_the_same_base_updates() {
        let i = installed_at("0.10.0+g111aaaa", Source::Local, false);
        let a = available("0.10.0+g111aaaa.dirty", Source::Local);
        assert_eq!(decide(Some(i), &a, false), Decision::Update);
    }

    #[test]
    fn same_base_updates_even_when_the_available_build_sha_sorts_first() {
        // `Version::cmp` (plain `Ord`) falls back to comparing build metadata lexicographically
        // once major/minor/patch/pre are equal, so a naive `cmp` here would call this pair
        // "older" just because "gaaa" < "gfff" as strings. `cmp_precedence` disregards build
        // metadata entirely, so same base version + different build is always Update.
        let i = installed_at("0.10.0+gfff", Source::Local, false);
        let a = available("0.10.0+gaaa", Source::Local);
        assert_eq!(decide(Some(i), &a, false), Decision::Update);
    }

    #[test]
    fn rebuilding_clean_over_a_previously_dirty_install_updates() {
        // "gaaa" is a string-prefix of "gaaa.dirty", so it sorts first — another case a plain
        // `cmp` would misread as the available build being older.
        let i = installed_at("0.10.0+gaaa.dirty", Source::Local, false);
        let a = available("0.10.0+gaaa", Source::Local);
        assert_eq!(decide(Some(i), &a, false), Decision::Update);
    }

    #[test]
    fn identical_build_metadata_skips() {
        let i = installed_at("0.10.0+g111aaaa", Source::Local, false);
        let a = available("0.10.0+g111aaaa", Source::Local);
        assert!(matches!(decide(Some(i), &a, false), Decision::Skip { .. }));
    }

    #[test]
    fn source_change_reinstalls() {
        let i = installed_at("0.10.0", Source::Local, false);
        let a = available("0.10.0", Source::Github);
        assert!(matches!(
            decide(Some(i), &a, false),
            Decision::Reinstall { .. }
        ));
    }

    #[test]
    fn legacy_no_version_updates() {
        let i = installed_at("0.0.0", Source::Github, true);
        let a = available("0.10.0", Source::Github);
        assert_eq!(decide(Some(i), &a, false), Decision::Update);
    }

    #[test]
    fn not_installed_installs() {
        let a = available("0.10.0", Source::Github);
        assert_eq!(decide(None, &a, false), Decision::Install);
    }

    #[test]
    fn force_reinstalls_every_installed_input() {
        let a = available("0.10.0", Source::Github);
        let cases = [
            installed_at("0.10.0", Source::Github, false), // equal
            installed_at("0.9.0", Source::Github, false),  // older than available
            installed_at("0.11.0", Source::Github, false), // newer than available
            installed_at("0.10.0", Source::Local, false),  // source change
            installed_at("0.0.0", Source::Github, true),   // legacy
        ];
        for i in cases {
            assert!(matches!(
                decide(Some(i), &a, true),
                Decision::Reinstall { .. }
            ));
        }
    }

    #[test]
    fn force_installs_when_not_installed() {
        let a = available("0.10.0", Source::Github);
        assert_eq!(decide(None, &a, true), Decision::Install);
    }

    // ── installed-version lookup ────────────────────────────────────────────

    #[test]
    fn version_file_wins_over_receipt_and_host_record() {
        let vf = VersionFile::new("claude", v("0.10.0"));
        let entry = ReceiptEntry {
            source: Source::Local,
            reference: "checkout".to_string(),
            marketplace: None,
            path: PathBuf::from("/x"),
            version: v("0.9.0"),
            installed_at: "t".to_string(),
        };
        let got = installed(Some(&vf), Some(&entry), Some("0.0.1"), Source::Github);
        assert_eq!(got.version, v("0.10.0"));
        assert!(!got.legacy);
    }

    #[test]
    fn receipt_wins_over_host_record_when_no_version_file() {
        let entry = ReceiptEntry {
            source: Source::Local,
            reference: "checkout".to_string(),
            marketplace: None,
            path: PathBuf::from("/x"),
            version: v("0.9.0"),
            installed_at: "t".to_string(),
        };
        let got = installed(None, Some(&entry), Some("0.0.1"), Source::Github);
        assert_eq!(got.version, v("0.9.0"));
        assert_eq!(got.source, Source::Local);
        assert!(got.legacy, "no version file makes this a legacy install");
    }

    #[test]
    fn host_record_wins_when_neither_file_nor_receipt_exist() {
        let got = installed(None, None, Some("0.0.1"), Source::Github);
        assert_eq!(got.version, v("0.0.1"));
        assert!(got.legacy);
    }

    #[test]
    fn nothing_at_all_is_the_legacy_zero_version() {
        let got = installed(None, None, None, Source::Github);
        assert_eq!(got.version, v("0.0.0"));
        assert!(got.legacy);
    }

    #[test]
    fn read_installed_reads_the_version_file_off_disk() {
        let dir = scratch("read-installed");
        let path = dir.join(".rtok-plugin-version");
        VersionFile::new("claude", v("0.10.0"))
            .write(&path)
            .unwrap();
        let receipt = Receipt::default();
        let got = read_installed(&path, &receipt, "claude", None, Source::Github).unwrap();
        assert_eq!(got.version, v("0.10.0"));
        assert!(!got.legacy);
    }

    #[test]
    fn read_installed_falls_back_when_the_version_file_is_missing() {
        let dir = scratch("read-installed-missing");
        let path = dir.join(".rtok-plugin-version");
        let receipt = Receipt::default();
        let got = read_installed(&path, &receipt, "claude", Some("0.0.1"), Source::Github).unwrap();
        assert_eq!(got.version, v("0.0.1"));
        assert!(got.legacy);
    }

    // ── Config override ─────────────────────────────────────────────────────

    #[test]
    fn receipt_path_uses_the_config_override() {
        let cfg = Config {
            plugin_receipt_path: Some(PathBuf::from("/tmp/rtok-test-receipt.json")),
            ..Config::default()
        };
        assert_eq!(
            receipt_path(&cfg),
            PathBuf::from("/tmp/rtok-test-receipt.json")
        );
    }

    // ── version file ────────────────────────────────────────────────────────

    #[test]
    fn version_file_round_trips() {
        let dir = scratch("roundtrip");
        let path = dir.join(".rtok-plugin-version");
        let file = VersionFile::new("claude", v("0.10.0"));
        file.write(&path).unwrap();
        assert_eq!(VersionFile::read(&path).unwrap(), file);
        let raw = fs::read_to_string(&path).unwrap();
        assert_eq!(
            raw,
            "{\"schema\":1,\"plugin\":\"claude\",\"version\":\"0.10.0\"}\n"
        );
    }

    #[test]
    fn unknown_schema_is_rejected() {
        let dir = scratch("bad-schema");
        let path = dir.join(".rtok-plugin-version");
        fs::write(
            &path,
            r#"{"schema":2,"plugin":"claude","version":"0.10.0"}"#,
        )
        .unwrap();
        let err = VersionFile::read(&path).unwrap_err().to_string();
        assert!(err.contains("schema 2"), "{err}");
        assert!(err.contains(&path.display().to_string()), "{err}");
    }

    #[test]
    fn invalid_semver_names_the_file() {
        let dir = scratch("bad-semver");
        let path = dir.join(".rtok-plugin-version");
        fs::write(
            &path,
            r#"{"schema":1,"plugin":"claude","version":"not-a-version"}"#,
        )
        .unwrap();
        let err = VersionFile::read(&path).unwrap_err().to_string();
        assert!(err.contains(&path.display().to_string()), "{err}");
    }

    // ── receipt ─────────────────────────────────────────────────────────────

    #[test]
    fn receipt_round_trips_in_a_temp_dir() {
        let dir = scratch("receipt");
        let path = dir.join("plugins.json");
        let mut receipt = Receipt::read(&path).unwrap(); // missing file reads as empty
        assert!(receipt.get("claude").is_none());
        receipt.upsert(
            "claude",
            ReceiptEntry {
                source: Source::Github,
                reference: "v0.10.0".to_string(),
                marketplace: Some("rtok".to_string()),
                path: PathBuf::from("/home/x/.claude/plugins/cache/rtok/rtok/0.10.0"),
                version: v("0.10.0"),
                installed_at: "2026-09-27T00:00:00Z".to_string(),
            },
        );
        receipt.write(&path).unwrap();
        let read_back = Receipt::read(&path).unwrap();
        assert_eq!(read_back.get("claude"), receipt.get("claude"));
        let mut mutated = read_back;
        assert!(mutated.delete("claude").is_some());
        assert!(mutated.get("claude").is_none());
    }

    // ── receipt path per OS ─────────────────────────────────────────────────

    #[cfg(target_os = "macos")]
    #[test]
    fn receipt_path_on_macos() {
        let home = Path::new("/Users/x");
        assert_eq!(
            default_receipt_path(home, None, None),
            home.join("Library/Application Support/rtok/plugins.json")
        );
    }

    #[cfg(windows)]
    #[test]
    fn receipt_path_on_windows() {
        let home = Path::new(r"C:\Users\x");
        assert_eq!(
            default_receipt_path(home, None, Some(r"C:\Users\x\AppData\Local")),
            PathBuf::from(r"C:\Users\x\AppData\Local").join("rtok/plugins.json")
        );
        // No LOCALAPPDATA: falls back to home\AppData\Local.
        assert_eq!(
            default_receipt_path(home, None, None),
            home.join("AppData/Local/rtok/plugins.json")
        );
    }

    #[cfg(not(any(target_os = "macos", windows)))]
    #[test]
    fn receipt_path_on_linux_xdg_state_home() {
        let home = Path::new("/home/x");
        assert_eq!(
            default_receipt_path(home, Some("/home/x/.local/state"), None),
            home.join(".local/state/rtok/plugins.json")
        );
        // No XDG_STATE_HOME: falls back to home/.local/state.
        assert_eq!(
            default_receipt_path(home, None, None),
            home.join(".local/state/rtok/plugins.json")
        );
    }

    // ── local_version ───────────────────────────────────────────────────────

    #[test]
    fn local_version_clean_describe() {
        let version = local_version(&v("0.10.0"), "12c7e91");
        assert_eq!(version.to_string(), "0.10.0+g12c7e91");
    }

    #[test]
    fn local_version_dirty_describe() {
        let version = local_version(&v("0.10.0"), "12c7e91-dirty");
        assert_eq!(version.to_string(), "0.10.0+g12c7e91.dirty");
    }

    #[test]
    fn local_version_tagged_describe() {
        let version = local_version(&v("0.10.0"), "v0.10.0-3-g12c7e91");
        assert_eq!(version.to_string(), "0.10.0+g12c7e91");
    }

    // ── Source::from_str ────────────────────────────────────────────────────

    #[test]
    fn source_from_str_accepts_any_case() {
        assert_eq!("github".parse::<Source>().unwrap(), Source::Github);
        assert_eq!("Local".parse::<Source>().unwrap(), Source::Local);
        assert_eq!(
            "MARKETPLACE".parse::<Source>().unwrap(),
            Source::Marketplace
        );
    }

    #[test]
    fn source_from_str_rejects_unknown() {
        let err = "npm".parse::<Source>().unwrap_err().to_string();
        assert!(err.contains("npm"), "{err}");
    }

    // ── available ────────────────────────────────────────────────────────────

    #[test]
    fn available_github_and_marketplace_read_as_the_running_version() {
        let running = v("0.10.0");
        for source in [Source::Github, Source::Marketplace] {
            let a = super::available(source, Path::new("/does/not/exist"), &running).unwrap();
            assert_eq!(a.version, running);
            assert_eq!(a.source, source);
        }
    }

    #[test]
    fn available_local_reads_the_tree_s_own_file_and_git_describe() {
        let dir = scratch("available-local");
        VersionFile::new("claude", v("0.10.0"))
            .write(&dir.join(".rtok-plugin-version"))
            .unwrap();
        std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(&dir)
            .status()
            .unwrap();
        std::process::Command::new("git")
            .args([
                "-c",
                "user.email=t@t",
                "-c",
                "user.name=t",
                "commit",
                "--allow-empty",
                "-q",
                "-m",
                "x",
            ])
            .current_dir(&dir)
            .status()
            .unwrap();
        let a = super::available(Source::Local, &dir, &v("99.0.0")).unwrap();
        assert_eq!(a.source, Source::Local);
        assert!(a.version.to_string().starts_with("0.10.0+g"), "{a:?}");
    }

    #[test]
    fn available_local_names_the_directory_when_git_fails() {
        let dir = scratch("available-local-no-git");
        VersionFile::new("claude", v("0.10.0"))
            .write(&dir.join(".rtok-plugin-version"))
            .unwrap();
        let err = super::available(Source::Local, &dir, &v("99.0.0"))
            .unwrap_err()
            .to_string();
        assert!(err.contains(&dir.display().to_string()), "{err}");
    }

    // ── now_iso ──────────────────────────────────────────────────────────────

    #[test]
    fn now_iso_looks_like_rfc3339_utc() {
        let ts = now_iso();
        assert_eq!(ts.len(), 20, "{ts}");
        assert!(ts.ends_with('Z'), "{ts}");
        assert_eq!(&ts[4..5], "-");
        assert_eq!(&ts[10..11], "T");
    }
}
