//! `rtok tui` — the terminal rendering of the one operator model (D23, P15).
//!
//! The TUI renders [`crate::web::model`]; it never queries the [`crate::store::Store`]
//! itself, and its tabs are [`model::pages`] verbatim, so there is no second page list
//! to let drift. This module owns the event loop (T15.1), [`app`] the pure state,
//! [`view`] the screen (T15.2; pages T15.3+). The TTY guard is T15.9.

mod app;
mod view;

use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyEventKind};

use crate::config::Config;
use crate::web::model;

/// `rtok tui`: alternate screen and raw mode until `q` / `Esc` / `Ctrl+C`. The terminal
/// is restored on every exit path — the loop's errors are returned, not panicked on.
pub fn run(cfg: Config) -> Result<()> {
    let mut terminal = ratatui::try_init().context("terminal init — is stdout a tty?")?;
    // `max(1)`: a zero cadence would busy-poll; `config validate` rejects it in the
    // file, this holds the line for `--tick-secs 0`.
    let tick = Duration::from_secs(cfg.tui.tick_secs.max(1));
    let mut app = app::App::new(&cfg);
    let res = event_loop(&mut terminal, &mut app, &cfg, tick);
    ratatui::restore();
    res
}

/// Draw, then wait up to `tick` for a key: a press updates the state, a timeout
/// re-reads the model — the same one-snapshot-per-tick shape `rtok web`'s socket serves.
fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut app::App,
    cfg: &Config,
    tick: Duration,
) -> Result<()> {
    loop {
        terminal.draw(|frame| view::draw(frame, app))?;
        if !event::poll(tick)? {
            app.refresh(model::snapshot(cfg));
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind == KeyEventKind::Press && app.key(key.code, key.modifiers) {
            return Ok(());
        }
    }
}
