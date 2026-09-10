//! Pure TUI state: which tab, what data, when it last refreshed — and, on the Plugins
//! tab, the row cursor and the toggle that writes `plugins.<id>.enabled` through `rtok
//! config set`'s writer (T15.4). No terminal and no clock reach this module from
//! outside a call — both arrive through [`super`]'s loop, which is what keeps the state
//! transitions unit-testable without a tty.

use crossterm::event::{KeyCode, KeyModifiers};

use crate::config::{Config, validate};
use crate::web::model::{self, Snapshot};

/// The TUI's whole state. The tabs are [`model::pages`] by reference — there is no
/// second list to let drift (D23); a page the model adds is a tab at the next `App::new`.
pub struct App {
    tabs: &'static [(&'static str, &'static str)],
    selected: usize,
    snapshot: Snapshot,
    /// Unix seconds of the last model re-read; the screen stamps it with `log::stamp`.
    updated: u64,
    /// The config the App reads the model through, so a toggle's re-read and every
    /// later tick serve the same copy (T15.4) — the loop no longer holds its own.
    cfg: Config,
    /// The Plugins tab's row cursor: which plugin a toggle would hit (T15.4).
    plugin_cursor: usize,
    /// The Plugins tab's status line: the last toggle's outcome (T15.4).
    plugin_status: String,
    /// The Calls page's own state (T15.5): which row is selected and whether its detail
    /// pane is open.
    calls: CallsState,
}

/// Selection and detail state of the Calls page (T15.5). The row list lives in the
/// snapshot (D23); this only remembers where the cursor sits on it.
#[derive(Default)]
struct CallsState {
    selected: usize,
    detail: bool,
}

impl App {
    pub fn new(cfg: &Config) -> Self {
        let tabs = model::pages();
        // `[tui] tab` names the opening tab; empty or unknown falls back to the first
        // page rather than failing the surface (fail open, D1).
        let selected = tabs
            .iter()
            .position(|(page, _)| *page == cfg.tui.tab)
            .unwrap_or(0);
        Self {
            tabs,
            selected,
            snapshot: model::snapshot(cfg),
            updated: crate::log::now(),
            cfg: cfg.clone(),
            plugin_cursor: 0,
            plugin_status: String::new(),
            calls: CallsState::default(),
        }
    }

    /// Tab titles straight off the model's page list.
    pub fn tab_names(&self) -> Vec<&'static str> {
        self.tabs.iter().map(|(page, _)| *page).collect()
    }

    /// The selected page's name.
    pub fn page(&self) -> &'static str {
        self.tabs[self.selected].0
    }

    /// The selected tab's index — where the highlight sits.
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// The Plugins tab's row cursor, clamped to the rows the model serves (T15.4).
    pub fn plugin_cursor(&self) -> usize {
        self.plugin_cursor
            .min(self.snapshot.plugins.len().saturating_sub(1))
    }

    /// The Plugins tab's status line: the last toggle's outcome, `""` until one (T15.4).
    pub fn plugin_status(&self) -> &str {
        &self.plugin_status
    }

    /// The last snapshot the model served.
    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    /// Unix seconds of the last tick; the screen stamps it with `log::stamp`.
    pub fn updated(&self) -> u64 {
        self.updated
    }

    /// A tick's new data: one model snapshot, timestamped now.
    pub fn refresh(&mut self, snapshot: Snapshot) {
        // The cursor stays on a row that still exists — a shorter page is not a panic.
        self.plugin_cursor = self
            .plugin_cursor
            .min(snapshot.plugins.len().saturating_sub(1));
        self.snapshot = snapshot;
        self.updated = crate::log::now();
    }

    /// The loop's tick: re-read the model through the config the App holds, so a
    /// toggle's re-read and the next tick cannot disagree (T15.4).
    pub fn tick(&mut self) {
        self.refresh(model::snapshot(&self.cfg));
    }

    /// The Calls page's selected row (T15.5), clamped to the rows it holds — a refresh
    /// that shrinks the page can move the selection, never past its end.
    pub fn calls_selected(&self) -> usize {
        self.calls
            .selected
            .min(self.snapshot.calls.len().saturating_sub(1))
    }

    /// Whether the Calls page's detail pane is open (T15.5).
    pub fn calls_detail(&self) -> bool {
        self.calls.detail
    }

    /// The Calls page's keys (T15.5): `Up`/`Down` walk the rows, `Enter`/`z` expand the
    /// selected one. Returns `true` when the key was consumed; the shell's keys
    /// (`q`, arrows, digits) are never reached here.
    fn calls_key(&mut self, code: KeyCode) -> bool {
        let last = self.snapshot.calls.len().saturating_sub(1);
        match code {
            KeyCode::Up => {
                self.calls.selected = self.calls.selected.saturating_sub(1);
                true
            }
            KeyCode::Down => {
                self.calls.selected = (self.calls.selected + 1).min(last);
                true
            }
            KeyCode::Enter | KeyCode::Char('z') if !self.snapshot.calls.is_empty() => {
                self.calls.detail = !self.calls.detail;
                true
            }
            _ => false,
        }
    }

    /// One key press; returns `true` when the loop should stop. `Left`/`Right` wrap,
    /// `1..=9` jump; the Calls page claims `Up`/`Down`/`Enter`/`z` (T15.5); the Plugins
    /// page claims the row keys (T15.4); everything else is the next page's to claim.
    pub fn key(&mut self, code: KeyCode, mods: KeyModifiers) -> bool {
        if mods.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
            return true;
        }
        if self.page() == "calls" && self.calls_key(code) {
            return false;
        }
        match code {
            KeyCode::Char('q') | KeyCode::Esc => true,
            KeyCode::Left => {
                self.step(-1);
                false
            }
            KeyCode::Right => {
                self.step(1);
                false
            }
            // The Plugins tab's row keys; every other page falls through (a Space there
            // reaches the digit arm, whose `to_digit` answer for ' ' is none).
            KeyCode::Up | KeyCode::Down | KeyCode::Enter | KeyCode::Char(' ')
                if self.page() == "plugins" =>
            {
                self.plugins_key(code);
                false
            }
            KeyCode::Char(c) => {
                // 1..=9, not 0: tabs count from one and nine is plenty for a tab bar.
                if let Some(d) = c.to_digit(10).filter(|d| (1..=9).contains(d)) {
                    self.select(d as usize - 1);
                }
                false
            }
            _ => false,
        }
    }

    /// The Plugins tab's keys (T15.4): `Up`/`Down` move the row cursor, `Space`/`Enter`
    /// toggle the selected plugin.
    fn plugins_key(&mut self, code: KeyCode) {
        let len = self.snapshot.plugins.len();
        match code {
            KeyCode::Up => {
                self.plugin_cursor = self.plugin_cursor.saturating_sub(1);
                self.plugin_status.clear();
            }
            KeyCode::Down => {
                if len > 0 {
                    self.plugin_cursor = (self.plugin_cursor + 1).min(len - 1);
                }
                self.plugin_status.clear();
            }
            KeyCode::Char(' ') | KeyCode::Enter => self.toggle_selected(),
            _ => {}
        }
    }

    /// Flip the selected plugin's `[plugins.<id>] enabled` (T15.4) through the one
    /// config writer — [`validate::set`], the API `rtok config set` uses — never a
    /// second one. The write always targets `<home>/config.toml`, as `config set`
    /// does; the row then re-renders off the effective value through the layering (an
    /// env override of the key still wins over the file, so the status line names what
    /// a fresh `rtok plugins` would print, not what the key press hoped for). A
    /// refusal — unknown plugin id, unwritable file — is a status line, not a crash:
    /// an operator surface fails open (D1).
    fn toggle_selected(&mut self) {
        let Some(id) = self.snapshot.plugins.get(self.plugin_cursor).map(|p| p.id) else {
            self.plugin_status = "no plugin to toggle".into();
            return;
        };
        let written = !self.snapshot.plugins[self.plugin_cursor].enabled;
        let key = format!("plugins.{id}.enabled");
        match validate::set(&self.cfg.home, &key, &written.to_string(), false) {
            Ok(_) => {
                let on = Config::load_from(&self.cfg.home)
                    .map(|reloaded| reloaded.plugin_enabled(id, true))
                    .unwrap_or(written);
                self.cfg.set_plugin_enabled(id, on);
                self.plugin_status = format!("{id} {}", if on { "on" } else { "off" });
                self.refresh(model::snapshot(&self.cfg));
            }
            Err(e) => self.plugin_status = format!("config set {key}: {e:#}"),
        }
    }

    /// Jump to tab `i`, modulo the list — out of range wraps rather than panics.
    fn select(&mut self, i: usize) {
        if !self.tabs.is_empty() {
            self.selected = i % self.tabs.len();
        }
    }

    fn step(&mut self, dir: i32) {
        if !self.tabs.is_empty() {
            let len = self.tabs.len() as i32;
            self.selected = (self.selected as i32 + dir).rem_euclid(len) as usize;
        }
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    pub(in crate::tui) fn config() -> Config {
        // A dir per call: one shared `rtok-tui-<pid>` was deleted by each parallel test
        // while another was still opening its store in it (os error 22).
        let dir = crate::testutil::tmp_dir("tui");
        let mut cfg = Config::load_from(&dir).expect("config");
        // Hermetic probes: the snapshot carries the doctor page (T15.6), so every
        // `App::new` would otherwise spawn this machine's MCP servers.
        cfg.doctor.settings_path = dir.join("missing-settings.json");
        cfg.doctor.claude_json = dir.join("missing-claude.json");
        cfg.doctor.mcp_json = dir.join("missing-mcp.json");
        cfg
    }

    /// The Plugins tab with the row cursor on `id` — where the T15.4 toggle tests
    /// start. `[tui] tab` picks the page (T15.8), so the helper says which tab it
    /// means rather than counting pages; a model without the row fails the test.
    pub(in crate::tui) fn cursor_on_plugin(cfg: &Config, id: &str) -> App {
        let mut cfg = cfg.clone();
        cfg.tui.tab = "plugins".into();
        // Same hermetic doctor paths as `config()` — App::new ticks the snapshot (T15.6).
        let home = cfg.home.clone();
        cfg.doctor.settings_path = home.join("missing-settings.json");
        cfg.doctor.claude_json = home.join("missing-claude.json");
        cfg.doctor.mcp_json = home.join("missing-mcp.json");
        let mut app = App::new(&cfg);
        assert_eq!(app.page(), "plugins");
        let i = app
            .snapshot()
            .plugins
            .iter()
            .position(|p| p.id == id)
            .unwrap_or_else(|| panic!("no {id} row on the Plugins page"));
        for _ in 0..i {
            app.key(KeyCode::Down, KeyModifiers::NONE);
        }
        app
    }

    /// D23: the tab bar is the model's page list, never a second one.
    #[test]
    fn tabs_are_the_model_pages() {
        let app = App::new(&config());
        assert_eq!(
            app.tab_names(),
            model::pages()
                .iter()
                .map(|(page, _)| *page)
                .collect::<Vec<_>>()
        );
        assert_eq!(app.page(), "overview");
    }

    /// T15.8: `[tui] tab` picks the opening tab; empty or unknown is the first page.
    #[test]
    fn tui_tab_picks_the_opening_tab() {
        let names: Vec<&str> = model::pages().iter().map(|(page, _)| *page).collect();
        assert!(names.len() >= 2, "needs two pages to choose between");
        let mut cfg = config();
        cfg.tui.tab = names[1].to_string();
        assert_eq!(App::new(&cfg).page(), names[1]);
        cfg.tui.tab = "no-such-page".to_string();
        assert_eq!(App::new(&cfg).page(), names[0]);
        cfg.tui.tab = String::new();
        assert_eq!(App::new(&cfg).page(), names[0]);
    }

    #[test]
    fn left_right_wrap_and_digits_jump() {
        let mut app = App::new(&config());
        let names = app.tab_names();
        let n = names.len();
        assert!(n >= 2, "the model serves at least the two T15.0 pages");
        app.key(KeyCode::Right, KeyModifiers::NONE);
        assert_eq!(app.page(), names[1]);
        app.key(KeyCode::Right, KeyModifiers::NONE);
        assert_eq!(app.page(), names[2 % n], "wraps past the last tab");
        app.key(KeyCode::Left, KeyModifiers::NONE);
        app.key(KeyCode::Left, KeyModifiers::NONE);
        assert_eq!(
            app.page(),
            names[0],
            "two steps each way land back on tab one"
        );
        app.key(KeyCode::Char('1'), KeyModifiers::NONE);
        assert_eq!(app.page(), names[0]);
        app.key(KeyCode::Char('2'), KeyModifiers::NONE);
        assert_eq!(app.page(), names[1]);
        app.key(KeyCode::Char('9'), KeyModifiers::NONE);
        assert_eq!(app.page(), names[8 % n], "a digit past the list wraps");
        app.key(KeyCode::Char('0'), KeyModifiers::NONE);
        assert_eq!(app.page(), names[8 % n], "0 is not a tab");
    }

    #[test]
    fn q_esc_and_ctrl_c_quit() {
        let mut app = App::new(&config());
        assert!(app.key(KeyCode::Char('q'), KeyModifiers::NONE));
        assert!(app.key(KeyCode::Esc, KeyModifiers::NONE));
        assert!(app.key(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(!app.key(KeyCode::Char('c'), KeyModifiers::NONE));
        assert!(!app.key(KeyCode::Char('x'), KeyModifiers::NONE));
    }

    #[test]
    fn refresh_serves_the_new_snapshot_and_stamps_it() {
        let cfg = config();
        let mut app = App::new(&cfg);
        let before = app.updated();
        app.refresh(model::snapshot(&cfg));
        assert!(app.updated() >= before);
        assert_eq!(
            app.snapshot().plugins.len(),
            model::snapshot(&cfg).plugins.len()
        );
    }

    /// T15.4: `Up`/`Down` move the Plugins tab's row cursor and clamp at both ends.
    #[test]
    fn plugins_cursor_moves_and_clamps() {
        let mut app = App::new(&config());
        app.key(KeyCode::Right, KeyModifiers::NONE);
        let len = app.snapshot().plugins.len();
        assert!(len >= 2, "the catalogue has rows to move between");
        assert_eq!(app.plugin_cursor(), 0);
        app.key(KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(app.plugin_cursor(), 0, "Up at the top stays put");
        app.key(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(app.plugin_cursor(), 1);
        for _ in 0..=len {
            app.key(KeyCode::Down, KeyModifiers::NONE);
        }
        assert_eq!(app.plugin_cursor(), len - 1, "Down clamps at the last row");
    }

    /// T15.4: the row keys are the Plugins page's, not global — on Overview they are
    /// nothing, and a Space is not a quit either.
    #[test]
    fn row_keys_do_nothing_off_the_plugins_page() {
        let mut app = App::new(&config());
        for code in [
            KeyCode::Up,
            KeyCode::Down,
            KeyCode::Enter,
            KeyCode::Char(' '),
        ] {
            assert!(!app.key(code, KeyModifiers::NONE), "{code:?} is not a quit");
            assert_eq!(app.page(), "overview", "{code:?} moved nothing");
        }
    }

    /// T15.4: Space flips `plugins.<id>.enabled` through `rtok config set`'s own writer
    /// (`validate::set` on `<home>/config.toml`), the row re-renders off the re-read
    /// model, and the next tick serves the same answer. Its own temp home — the write
    /// is real, just never the operator's file.
    #[test]
    fn space_toggles_the_selected_plugin_through_config_set() {
        let dir = std::env::temp_dir().join(format!("rtok-tui-toggle-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let cfg = Config::load_from(&dir).expect("config");
        let mut app = cursor_on_plugin(&cfg, "cmd");
        let row = |app: &App| {
            app.snapshot()
                .plugins
                .iter()
                .find(|p| p.id == "cmd")
                .expect("cmd row")
                .enabled
        };
        assert!(Config::load_from(&dir).unwrap().plugin_enabled("cmd", true));
        app.key(KeyCode::Char(' '), KeyModifiers::NONE);
        assert!(
            !Config::load_from(&dir).unwrap().plugin_enabled("cmd", true),
            "the file changed — the write went through config set's writer"
        );
        assert!(!row(&app), "the row re-read the write");
        assert!(
            app.plugin_status().contains("cmd off"),
            "the status names it"
        );
        app.tick();
        assert!(
            !row(&app),
            "the next tick keeps the write, not the launch copy"
        );
        app.key(KeyCode::Enter, KeyModifiers::NONE); // Enter toggles too
        assert!(
            Config::load_from(&dir).unwrap().plugin_enabled("cmd", true),
            "toggled back on"
        );
        assert!(row(&app));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// T15.5: the Calls page claims `Up`/`Down`/`Enter`/`z` — the selection clamps to
    /// the rows it holds and the detail toggles — while the shell's keys keep working
    /// on the same page.
    #[test]
    fn calls_page_claims_its_keys_and_clamps_the_selection() {
        let cfg = config();
        let mut app = App::new(&cfg);
        let calls = model::pages()
            .iter()
            .position(|(p, _)| *p == "calls")
            .unwrap();
        app.select(calls);
        assert!(app.snapshot().calls.is_empty(), "nothing seeded");
        assert_eq!(app.calls_selected(), 0);
        app.key(KeyCode::Down, KeyModifiers::NONE);
        app.key(KeyCode::Up, KeyModifiers::NONE);
        app.key(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(app.calls_selected(), 0, "clamped on an empty page");
        assert!(!app.calls_detail(), "Enter does nothing with no rows");

        // Two rows: the selection walks and clamps at both ends, Enter/z toggle.
        app.refresh({
            let mut snap = model::snapshot(&cfg);
            snap.calls = vec![row(7), row(8)];
            snap
        });
        app.key(KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(app.calls_selected(), 0, "clamped at the newest row");
        app.key(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(app.calls_selected(), 1);
        app.key(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(app.calls_selected(), 1, "clamped at the oldest row");
        app.key(KeyCode::Enter, KeyModifiers::NONE);
        assert!(app.calls_detail());
        app.key(KeyCode::Char('z'), KeyModifiers::NONE);
        assert!(!app.calls_detail(), "z closes what Enter opened");

        // A refresh that shrinks the page moves the selection back inside it.
        app.refresh({
            let mut snap = model::snapshot(&cfg);
            snap.calls = vec![row(9)];
            snap
        });
        assert_eq!(app.calls_selected(), 0);

        // The shell's keys still work on the Calls page.
        assert!(app.key(KeyCode::Char('q'), KeyModifiers::NONE), "q quits");
    }

    /// A bare ledger row for the selection test — the view's tests seed real ones.
    fn row(id: i32) -> crate::store::CallRow {
        crate::store::CallRow {
            id,
            ts: 0,
            session: "s".into(),
            surface: "hook".into(),
            kind: "hook".into(),
            plugin: None,
            name: None,
            parent_id: None,
            ms: None,
            ok: 1,
            error: None,
            host: None,
            provider: None,
            model: None,
            api: None,
            input: None,
            cache_create: None,
            cache_read: None,
            output: None,
        }
    }
}
