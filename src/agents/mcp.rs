//! Shared MCP config-entry core (T275, the start of it — full table-driven core lands in
//! T277). `has_entry` is the read side every host's `installed()` uses to say whether its own
//! config file really carries rtok's MCP server, instead of a raw `contains("\"rtok\"")` on the
//! file text, which a `name` merely mentioned elsewhere (a comment, an unrelated value) would
//! wrongly count.
//!
//! T278: [`rows`] is the per-surface MCP status `agents info`, `agents list`, `--json`, the web
//! model and `installed_hosts` all read, built on `rtok_mcp::status` (T277).

use std::path::{Path, PathBuf};

use rtok_mcp::config::Fs;
use rtok_mcp::registry::RTOK;
use rtok_mcp::spec::{Client, DuplicateName, EntryShape, Format, McpSpec};
use rtok_mcp::status::Entry;
use serde_json::Value;
use toml_edit::{DocumentMut, Item};

use super::Kind;
use crate::config::Config;

/// Whether `<key>.<name>` exists as a JSON object in an already-parsed `value` — never a
/// substring match. A `key` that is not an object, or a `name` that exists but is not itself an
/// object (or is mentioned only elsewhere) reads as `false` — fail open, never a panic. Shared
/// by `has_entry` (reads a config file off disk) and doctor.rs's own missing-entry check, so the
/// lookup lives in one place.
pub(crate) fn entry_in(value: &Value, key: &str, name: &str) -> bool {
    value
        .get(key)
        .and_then(|servers| servers.get(name))
        .is_some_and(Value::is_object)
}

/// Whether `<key>.<name>` exists as a JSON object in the JSONC file at `path` — never a
/// substring match on the raw text. An absent, unreadable, or unparsable file, a `key` that is
/// not an object, or a `name` that exists but is not itself an object (or is mentioned only
/// elsewhere in the file) all read as `false` — fail open, never a panic.
pub(crate) fn has_entry(path: &Path, key: &str, name: &str) -> bool {
    let Ok(value) = super::jsonc::parse(&super::read(path)) else {
        return false;
    };
    entry_in(&value, key, name)
}

/// [`has_entry`]'s TOML equivalent, for Codex's and Grok's `~/.codex/config.toml`-shaped
/// config: whether `[<table>.<name>]` exists as a table in the TOML document at `path` —
/// never a substring match on the raw text. An absent, unreadable, or unparsable file, a
/// `table` that is not itself a table, or a `name` that exists but is not a table (or is
/// mentioned only elsewhere) all read as `false` — fail open, never a panic.
pub(crate) fn has_toml_entry(path: &Path, table: &str, name: &str) -> bool {
    let Ok(doc) = super::read(path).parse::<DocumentMut>() else {
        return false;
    };
    doc.get(table)
        .and_then(Item::as_table)
        .and_then(|t| t.get(name))
        .is_some_and(Item::is_table)
}

/// `item` as a [`Value`], so a TOML entry (Codex's, Grok's `[mcp_servers.rtok]`) can run
/// through [`rtok_agent_sdk::judge_owned`] the same way the JSON hosts' entries do (T246.5).
/// Covers the shapes an `[mcp_servers.rtok]` table can hold.
pub(crate) fn toml_item_to_json(item: &Item) -> Value {
    match item {
        Item::None => Value::Null,
        Item::Value(v) => toml_value_to_json(v),
        Item::Table(t) => t
            .iter()
            .map(|(k, v)| (k.to_string(), toml_item_to_json(v)))
            .collect(),
        Item::ArrayOfTables(_) => Value::Null,
    }
}

fn toml_value_to_json(v: &toml_edit::Value) -> Value {
    match v {
        toml_edit::Value::String(s) => Value::String(s.value().clone()),
        toml_edit::Value::Integer(i) => Value::Number((*i.value()).into()),
        toml_edit::Value::Float(f) => {
            serde_json::Number::from_f64(*f.value()).map_or(Value::Null, Value::Number)
        }
        toml_edit::Value::Boolean(b) => Value::Bool(*b.value()),
        toml_edit::Value::Datetime(d) => Value::String(d.value().to_string()),
        toml_edit::Value::Array(a) => a.iter().map(toml_value_to_json).collect(),
        toml_edit::Value::InlineTable(t) => t
            .iter()
            .map(|(k, v)| (k.to_string(), toml_value_to_json(v)))
            .collect(),
    }
}

/// The server key most hosts keep their MCP servers under.
const SERVERS: &str = "mcpServers";

/// One MCP surface of a host variant (T278): the [`McpSpec`] [`rtok_mcp::status`] checks, and
/// what else serves `"rtok"` there once installed (a plugin, Grok's Claude import). Built per
/// variant, so a plugin only counts for the surface it serves: the Claude Code plugin never
/// reaches Claude Desktop's row.
pub(crate) struct McpSurface {
    pub label: &'static str,
    pub spec: McpSpec,
    pub plugin: Option<&'static str>,
}

/// `rtok_mcp`'s [`Entry`] as `agents info --json` and the web model spell it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryState {
    Present,
    Missing,
    Stale,
}

/// One [`McpSurface`]'s status: `surface`, `entry`, `plugin` (T278), plus the file and, for a
/// stale entry or an unreadable file, what is wrong.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct McpRow {
    pub surface: &'static str,
    pub file: String,
    pub entry: EntryState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
    pub plugin: Option<&'static str>,
}

impl McpRow {
    /// rtok's server reaches this surface as rtok would run it: its own entry, or a plugin.
    pub fn serves(&self) -> bool {
        self.entry == EntryState::Present || self.plugin.is_some()
    }

    /// The `mcp` module reads installed: an `rtok` entry exists (even a stale one, which
    /// `agents update` rewrites) or a plugin serves it.
    pub fn found(&self) -> bool {
        self.entry != EntryState::Missing || self.plugin.is_some()
    }

    /// `entry <file>`, `stale (<diff>)`, `missing (<file> has no "rtok")`, and
    /// `plugin <name> (serves "rtok")` when a plugin serves the surface too.
    pub fn text(&self) -> String {
        let name = RTOK.name;
        let entry = match (self.entry, &self.diff) {
            (EntryState::Present, _) => Some(format!("entry {}", self.file)),
            (EntryState::Stale, d) => Some(format!("stale ({})", d.as_deref().unwrap_or(""))),
            (EntryState::Missing, _) if self.plugin.is_some() => None,
            (EntryState::Missing, Some(err)) => Some(format!("missing ({err})")),
            (EntryState::Missing, None) => {
                let file = Path::new(&self.file).file_name().unwrap_or_default();
                Some(format!("missing ({} has no \"{name}\")", file.display()))
            }
        };
        let plugin = self
            .plugin
            .map(|p| format!("plugin {p} (serves \"{name}\")"));
        entry
            .into_iter()
            .chain(plugin)
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// `✓ mcp     desktop  entry ~/…`, one line per row, in [`super::module_lines`]' colours.
pub(crate) fn lines(rows: &[McpRow], indent: &str) -> String {
    use super::ModuleState;
    rows.iter()
        .map(|r| {
            let state = if r.serves() {
                ModuleState::Installed
            } else {
                ModuleState::NotInstalled
            };
            state.line(
                indent,
                &format!("mcp     {}  {}", r.surface, r.text()),
                true,
            )
        })
        .collect()
}

/// Every MCP surface `agent`'s `kind` variant reads, with the file its own installer writes.
/// Empty for a host with no MCP entry of its own (aider, pi, antigravity): its
/// [`super::Agent::installed`] stays the answer there. The rtok plugin serves a surface only
/// where the host says its plugin carries MCP ([`super::Agent::plugin_surfaces`]) and this
/// variant has it installed; Grok's `[compat.claude]` import of `~/.claude.json` serves too.
pub(crate) fn surfaces(agent: &dyn super::Agent, cfg: &Config, kind: Kind) -> Vec<McpSurface> {
    use super::{claude, cline, codewhale, copilot, cursor, devin, gemini, grok, kimi, opencode};
    let (host, s, desktop) = (agent.id(), &cfg.setup, kind == Kind::Desktop);
    let plugin = (agent
        .plugin_surfaces()
        .contains(&rtok_plugin_sdk::Surface::Mcp)
        && agent.installed(cfg, kind).contains(&"plugin"))
    .then_some(RTOK.name);
    let row = |label, file, key| surface(host, label, kind, file, key, plugin);
    if host == "vscode" {
        return [("stable", false), ("insiders", true)]
            .map(|(label, i)| row(label, super::vscode::mcp_path(cfg, i), "servers"))
            .into();
    }
    let (file, key) = match host {
        "claude" if desktop => (claude::desktop_path(), SERVERS),
        "claude" => (cfg.doctor.claude_json.clone(), SERVERS),
        "cline" if desktop => (cline::ext_mcp_path(cfg), SERVERS),
        "cline" => (s.cline.mcp_path.clone(), SERVERS),
        "codewhale" => (codewhale::mcp_path(cfg), SERVERS),
        "codex" => (s.codex.config_path.clone(), "mcp_servers"),
        "copilot" => (copilot::mcp_path(cfg), SERVERS),
        "cursor" => (cursor::mcp_path(cfg), SERVERS),
        "devin" => (devin::mcp_path(cfg), SERVERS),
        "gemini" => (gemini::settings_path(cfg), SERVERS),
        "grok" => {
            let mut grok_row = row(kind.as_str(), s.grok.config_path.clone(), "mcp_servers");
            let import = grok::covered(cfg, "mcp").then_some("[compat.claude] import");
            grok_row.plugin = grok_row.plugin.or(import);
            return vec![grok_row];
        }
        "kilo" => (s.kilo.config_path.clone(), "mcp"),
        "kimi" => (kimi::mcp_path(cfg), SERVERS),
        "mimo" => (s.mimo.config_path.clone(), "mcp"),
        "omp" => (s.omp.mcp_path.clone(), SERVERS),
        "opencode" if desktop => (opencode::desktop_path(), "mcp"),
        "opencode" => (s.opencode.config_path.clone(), "mcp"),
        "windsurf" => (s.windsurf.config_path.clone(), SERVERS),
        "zcode" => (s.zcode.config_path.clone(), "mcp.servers"),
        "zed" => (s.zed.config_path.clone(), "context_servers"),
        _ => return Vec::new(),
    };
    vec![row(kind.as_str(), file, key)]
}

fn surface(
    host: &'static str,
    label: &'static str,
    kind: Kind,
    file: PathBuf,
    key: &str,
    plugin: Option<&'static str>,
) -> McpSurface {
    let format = if file.extension().is_some_and(|e| e == "toml") {
        Format::Toml
    } else {
        Format::Jsonc // a strict-JSON file parses as JSONC too; status never writes
    };
    let spec = McpSpec {
        host,
        client: match kind {
            Kind::Cli => Client::Cli,
            Kind::Desktop => Client::Desktop,
        },
        config_path: file,
        format,
        key_path: McpSpec::key_path(key),
        server: RTOK,
        entry_shape: EntryShape::Command,
        duplicate_name: DuplicateName::ShowsBoth,
        plugin_serves: plugin.map(|_| RTOK.name),
    };
    McpSurface {
        label,
        spec,
        plugin,
    }
}

/// [`rtok_mcp::status`] for one surface. Its strict equality holds for `{command: "rtok",
/// args: ["mcp"]}`; a host that writes an absolute binary or extra keys (`type`, `tools`,
/// `enabled`) reads as stale there, so a stale entry is judged again by its argv alone.
pub(crate) fn status(fs: &impl Fs, s: &McpSurface) -> McpRow {
    let (entry, diff) = match rtok_mcp::status::status(fs, &s.spec).map(|st| st.entry) {
        Ok(Entry::Present) => (EntryState::Present, None),
        Ok(Entry::Missing) => (EntryState::Missing, None),
        Ok(Entry::Stale(_)) => match rtok_mcp::config::read_entry(fs, &s.spec) {
            Ok(Some(have)) => match stale_diff(&have) {
                None => (EntryState::Present, None),
                d => (EntryState::Stale, d),
            },
            _ => (EntryState::Missing, None),
        },
        Err(e) => (EntryState::Missing, Some(format!("{e:#}"))),
    };
    McpRow {
        surface: s.label,
        file: tilde(&s.spec.config_path),
        entry,
        diff,
        plugin: s.plugin,
    }
}

/// [`status`] of every surface `agent`'s `kind` variant reads, off the real disk.
pub(crate) fn rows(agent: &dyn super::Agent, cfg: &Config, kind: Kind) -> Vec<McpRow> {
    surfaces(agent, cfg, kind)
        .iter()
        .map(|s| status(&Disk, s))
        .collect()
}

/// The read-only disk [`rows`] goes through: status never writes.
struct Disk;

impl Fs for Disk {
    fn read(&self, path: &Path) -> Option<Vec<u8>> {
        std::fs::read(path).ok()
    }
    fn write(&mut self, path: &Path, _: Vec<u8>) -> anyhow::Result<()> {
        anyhow::bail!("{}: MCP status is read-only", path.display())
    }
}

/// `None` when `have` runs `rtok mcp` (any path to the rtok binary, argv as `command` +
/// `args` or as one `command` array); otherwise what it runs instead.
fn stale_diff(have: &Value) -> Option<String> {
    let strs = |v: Option<&Value>| -> Vec<String> {
        v.and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    };
    let argv = match have.get("command") {
        Some(Value::String(c)) => [vec![c.clone()], strs(have.get("args"))].concat(),
        c => strs(c),
    };
    let ours = argv.split_first().is_some_and(|(bin, args)| {
        super::is_rtok_bin(bin)
            && args
                .iter()
                .map(String::as_str)
                .eq(RTOK.args.iter().copied())
    });
    let want = [&[RTOK.command], RTOK.args].concat().join(" ");
    (!ours).then(|| format!("runs `{}`, want `{want}`", argv.join(" ")))
}

/// `path` with the home directory spelled `~`.
fn tilde(path: &Path) -> String {
    let home = super::home_dir();
    match path.strip_prefix(&home) {
        Ok(rest) if !home.as_os_str().is_empty() => format!("~/{}", rest.display()),
        _ => path.display().to_string(),
    }
}

/// T275/D33 checks (a)-(e) for one host, shared so every host runs the same sequence: with
/// the host's plugin already in place, (d) no entry reads as no `mcp`; (a) install writes it;
/// (b) update keeps it; (e) remove leaves an entry the user edited (`edit` adds `--extra` to
/// it in `file`) with a `leave <label>` line; (c) once `reseed` writes rtok's own entry again,
/// remove takes it out.
#[cfg(test)]
pub(crate) fn assert_entry_lifecycle(
    agent: &dyn super::Agent,
    cfg: &crate::config::Config,
    kind: super::Kind,
    label: &str,
    file: &Path,
    edit: impl Fn(),
    reseed: impl Fn(),
) {
    use super::Mode;
    let has = || agent.installed(cfg, kind).contains(&"mcp");
    assert!(!has(), "(d) no {label} yet");
    agent.apply(cfg, kind, Mode::Install).unwrap();
    assert!(has(), "(a) install writes {label}");
    let lines = agent.apply(cfg, kind, Mode::Update).unwrap();
    let removed = format!("- {label}");
    assert!(!lines.iter().any(|l| l.contains(&removed)), "(b) {lines:?}");
    assert!(has(), "(b) update keeps {label}");
    edit();
    let lines = agent.apply(cfg, kind, Mode::Remove).unwrap();
    let leave = format!("leave {label}");
    assert!(lines.iter().any(|l| l.starts_with(&leave)), "(e) {lines:?}");
    assert!(
        super::read(file).contains("--extra"),
        "(e) edited entry survives"
    );
    reseed();
    assert!(has(), "(c) reseeded {label}");
    agent.apply(cfg, kind, Mode::Remove).unwrap();
    assert!(!has(), "(c) remove takes {label} out");
}

/// [`assert_entry_lifecycle`] for a host whose entry is `rtok` under the dotted JSON `key` in
/// `file`; `reseed` writes rtok's own entry again.
#[cfg(test)]
pub(crate) fn assert_json_entry_lifecycle(
    agent: &dyn super::Agent,
    cfg: &crate::config::Config,
    kind: super::Kind,
    file: &Path,
    key: &str,
    reseed: impl Fn() -> anyhow::Result<String>,
) {
    let label = format!("{key}.rtok");
    let edit = || edit_json_args(file, key);
    assert_entry_lifecycle(agent, cfg, kind, &label, file, edit, || {
        reseed().unwrap();
    });
}

/// The user edit [`assert_entry_lifecycle`] expects, for a JSON config: `--extra` appended to
/// the `rtok` entry's `args` under the dotted `key`.
#[cfg(test)]
pub(crate) fn edit_json_args(file: &Path, key: &str) {
    let mut doc: Value = serde_json::from_str(&super::read(file)).unwrap();
    let entry = key.split('.').fold(&mut doc, |v, k| &mut v[k]);
    entry["rtok"]["args"] = serde_json::json!(["mcp", "--extra"]);
    std::fs::write(file, doc.to_string()).unwrap();
}

/// [`assert_entry_lifecycle`] for a host whose entry is `rtok` under the TOML `table` in
/// `file` (Codex's, Grok's `[mcp_servers.rtok]`); `reseed` writes rtok's own entry again.
#[cfg(test)]
pub(crate) fn assert_toml_entry_lifecycle(
    agent: &dyn super::Agent,
    cfg: &crate::config::Config,
    kind: super::Kind,
    file: &Path,
    table: &str,
    reseed: impl Fn() -> anyhow::Result<String>,
) {
    let label = format!("{table}.rtok");
    let edit = || edit_toml_args(file, table);
    assert_entry_lifecycle(agent, cfg, kind, &label, file, edit, || {
        reseed().unwrap();
    });
}

/// The user edit [`assert_entry_lifecycle`] expects, for a TOML config: `--extra` appended to
/// the `rtok` entry's `args` under the dotted `table`.
#[cfg(test)]
pub(crate) fn edit_toml_args(file: &Path, table: &str) {
    let mut doc: DocumentMut = super::read(file).parse().unwrap();
    let mut parts = table.split('.');
    let mut entry = &mut doc[parts.next().unwrap()];
    for part in parts {
        entry = &mut entry[part];
    }
    entry["rtok"]["args"] = toml_edit::value(toml_edit::Array::from_iter(["mcp", "--extra"]));
    std::fs::write(file, doc.to_string()).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp_file(name: &str, contents: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("rtok-mcp-has-entry-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn present_entry_is_true() {
        let path = tmp_file(
            "present",
            r#"{"mcpServers": {"rtok": {"command": "rtok", "args": ["mcp"]}}}"#,
        );
        assert!(has_entry(&path, "mcpServers", "rtok"));
    }

    #[test]
    fn missing_key_name_or_file_is_false() {
        let path = tmp_file("missing", r#"{"mcpServers": {"other": {"command": "x"}}}"#);
        assert!(!has_entry(&path, "mcpServers", "rtok"));
        assert!(!has_entry(&path, "otherKey", "rtok"));
        assert!(!has_entry(
            Path::new("/does/not/exist/rtok-t275.json"),
            "mcpServers",
            "rtok"
        ));
    }

    /// `"rtok"` shows up in a comment and as an unrelated value, never as `mcpServers.rtok`
    /// itself — a substring match on the raw text would wrongly say yes (the bug this
    /// function replaces).
    #[test]
    fn name_mentioned_elsewhere_in_the_file_is_not_an_entry() {
        let path = tmp_file(
            "elsewhere",
            r#"{
                // rtok
                "notes": "rtok lives here too",
                "mcpServers": {"other": {"command": "rtok"}}
            }"#,
        );
        assert!(!has_entry(&path, "mcpServers", "rtok"));
    }

    #[test]
    fn invalid_file_is_false() {
        let path = tmp_file("invalid", "{ this is not json");
        assert!(!has_entry(&path, "mcpServers", "rtok"));
    }

    fn tmp_toml(name: &str, contents: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rtok-mcp-has-toml-entry-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn toml_present_entry_is_true() {
        let path = tmp_toml(
            "present",
            "[mcp_servers.rtok]\ncommand = \"rtok\"\nargs = [\"mcp\"]\n",
        );
        assert!(has_toml_entry(&path, "mcp_servers", "rtok"));
    }

    #[test]
    fn toml_missing_table_name_or_file_is_false() {
        let path = tmp_toml("missing", "[mcp_servers.other]\ncommand = \"x\"\n");
        assert!(!has_toml_entry(&path, "mcp_servers", "rtok"));
        assert!(!has_toml_entry(&path, "other_table", "rtok"));
        assert!(!has_toml_entry(
            Path::new("/does/not/exist/rtok-t275.toml"),
            "mcp_servers",
            "rtok"
        ));
    }

    /// `"rtok"` shows up as an unrelated string value, never as `[mcp_servers.rtok]` itself.
    #[test]
    fn toml_name_mentioned_elsewhere_is_not_an_entry() {
        let path = tmp_toml(
            "elsewhere",
            "notes = \"rtok lives here too\"\n\n[mcp_servers.other]\ncommand = \"rtok\"\n",
        );
        assert!(!has_toml_entry(&path, "mcp_servers", "rtok"));
    }

    #[test]
    fn toml_invalid_file_is_false() {
        let path = tmp_toml("invalid", "this is not = [valid toml");
        assert!(!has_toml_entry(&path, "mcp_servers", "rtok"));
    }

    impl Fs for crate::testutil::Vfs {
        fn read(&self, path: &Path) -> Option<Vec<u8>> {
            crate::testutil::Vfs::read(self, &path.to_string_lossy()).map(<[u8]>::to_vec)
        }
        fn write(&mut self, path: &Path, bytes: Vec<u8>) -> anyhow::Result<()> {
            crate::testutil::Vfs::write(self, path.to_string_lossy(), bytes);
            Ok(())
        }
    }

    /// Plugin detection reads these; point them nowhere so the test never reads the real home.
    fn vfs_cfg() -> Config {
        let mut c = Config::default();
        let root = Path::new("/nonexistent/rtok-t278");
        c.setup.gemini.dir = root.join("gemini");
        c.setup.grok.config_path = root.join("grok/config.toml");
        c.setup.claude.settings_path = root.join("claude/settings.json");
        c.doctor.claude_json = root.join("claude.json");
        c
    }

    /// T278: every host × variant surface, each combination of entry and plugin, written through
    /// the same `rtok_mcp` config layer; the printed line and the `--json` row follow the file.
    #[test]
    fn every_host_surface_reports_what_its_file_holds() {
        use EntryState::*;
        let cfg = vfs_cfg();
        let mut seen = 0;
        for id in super::super::HOSTS {
            let host = super::super::host(id).unwrap();
            for v in host.variants() {
                for s in &mut surfaces(host, &cfg, v.kind) {
                    seen += 1;
                    let local = s.spec.key_path == ["mcp"];
                    let ours = if local {
                        serde_json::json!({"type": "local", "command": ["/opt/bin/rtok", "mcp"], "enabled": true})
                    } else {
                        serde_json::json!({"type": "stdio", "command": "/opt/bin/rtok", "args": ["mcp"]})
                    };
                    let edited = serde_json::json!({"command": "rtok", "args": ["mcp", "--extra"]});
                    let name = s
                        .spec
                        .config_path
                        .file_name()
                        .unwrap()
                        .display()
                        .to_string();
                    let file = tilde(&s.spec.config_path);
                    let cases: [(Option<&Value>, bool, EntryState, String); 5] = [
                        (
                            None,
                            false,
                            Missing,
                            format!("missing ({name} has no \"rtok\")"),
                        ),
                        (Some(&ours), false, Present, format!("entry {file}")),
                        (
                            Some(&edited),
                            false,
                            Stale,
                            "stale (runs `rtok mcp --extra`, want `rtok mcp`)".into(),
                        ),
                        (None, true, Missing, "plugin rtok (serves \"rtok\")".into()),
                        (
                            Some(&ours),
                            true,
                            Present,
                            format!("entry {file}, plugin rtok (serves \"rtok\")"),
                        ),
                    ];
                    for (entry, plugin, want, text) in cases {
                        let mut fs = crate::testutil::Vfs::new();
                        if let Some(e) = entry {
                            rtok_mcp::config::write_entry(&mut fs, &s.spec, e).unwrap();
                        }
                        s.plugin = plugin.then_some("rtok");
                        let row = status(&fs, s);
                        let at = format!("{id} {} {}", s.label, row.file);
                        assert_eq!((row.entry, row.text()), (want, text), "{at}");
                        assert_eq!(row.serves(), want == Present || plugin, "{at}");
                        let json = serde_json::to_value(&row).unwrap();
                        assert_eq!(json["surface"], s.label, "{at}");
                        assert_eq!(json["entry"], serde_json::to_value(want).unwrap(), "{at}");
                        assert_eq!(json["plugin"], serde_json::json!(s.plugin), "{at}");
                        let line = lines(&[row], "");
                        assert!(line.contains(&format!("mcp     {}  ", s.label)), "{line}");
                    }
                }
            }
        }
        assert!(seen >= 20, "{seen} surfaces");
    }

    /// Before T275 the Code plugin made Desktop read as installed; with it installed and a
    /// Desktop file that has no `rtok`, Desktop is `missing`.
    #[test]
    fn claude_code_plugin_never_makes_desktop_installed() {
        let (mut cfg, plugins) = super::super::test_scratch_cfg(
            "claude",
            "t278-desktop",
            "plugins/installed_plugins.json",
            true,
            |_, _| {},
        );
        cfg.setup.claude.settings_path = plugins.parent().unwrap().with_file_name("settings.json");
        fs::create_dir_all(plugins.parent().unwrap()).unwrap();
        fs::write(&plugins, r#"{"plugins": {"rtok@rtok": [{}]}}"#).unwrap();
        assert!(super::super::claude::plugin_installed(&cfg));

        let desktop = surfaces(&super::super::claude::Claude, &cfg, Kind::Desktop);
        let mut vfs = crate::testutil::Vfs::new();
        let path = desktop[0].spec.config_path.to_string_lossy().to_string();
        vfs.write(path, r#"{"mcpServers": {"other": {"command": "x"}}}"#);
        let row = status(&vfs, &desktop[0]);
        assert_eq!(
            (row.surface, row.entry, row.plugin),
            ("desktop", EntryState::Missing, None)
        );
        assert_eq!(
            row.text(),
            "missing (claude_desktop_config.json has no \"rtok\")"
        );
        assert!(!row.found());
        assert!(lines(&[row], "").contains("mcp     desktop  missing"));
    }

    #[test]
    fn toml_item_to_json_converts_a_server_table() {
        let doc: DocumentMut = "[mcp_servers.rtok]\ncommand = \"rtok\"\nargs = [\"mcp\"]\n"
            .parse()
            .unwrap();
        let item = doc["mcp_servers"]["rtok"].clone();
        assert_eq!(
            toml_item_to_json(&item),
            serde_json::json!({"command": "rtok", "args": ["mcp"]})
        );
    }
}
