//! Pure TUI state: which tab, what data, when it last refreshed. No terminal and no
//! clock reach this module from outside a call — both arrive through [`super`]'s loop,
//! which is what keeps the state transitions unit-testable without a tty.

use crossterm::event::{KeyCode, KeyModifiers};

use crate::config::Config;
use crate::web::model::{self, Snapshot};

/// The TUI's whole state. The tabs are [`model::pages`] by reference — there is no
/// second list to let drift (D23); a page the model adds is a tab at the next `App::new`.
pub struct App {
    tabs: &'static [(&'static str, &'static str)],
    selected: usize,
    snapshot: Snapshot,
    /// Unix seconds of the last model re-read; the screen stamps it with `log::stamp`.
    updated: u64,
}

impl App {
    pub fn new(cfg: &Config) -> Self {
        Self {
            tabs: model::pages(),
            selected: 0,
            snapshot: model::snapshot(cfg),
            updated: crate::log::now(),
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
        self.snapshot = snapshot;
        self.updated = crate::log::now();
    }

    /// One key press; returns `true` when the loop should stop. `Left`/`Right` wrap,
    /// `1..=9` jump, everything else is the page's to claim (T15.3+).
    pub fn key(&mut self, code: KeyCode, mods: KeyModifiers) -> bool {
        if mods.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
            return true;
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
        let dir = std::env::temp_dir().join(format!("rtok-tui-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Config::load_from(&dir).expect("config")
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
}
