//! `rtok tui` — the terminal rendering of the one operator model (D23, P15).
//!
//! The TUI renders [`crate::web::model`]; it never queries the [`crate::store::Store`]
//! itself, and its tabs are [`model::pages`] verbatim, so there is no second page list
//! to let drift. This module owns the event loop (T15.1), [`app`] the pure state,
//! [`view`] the screen (T15.2; pages T15.3+). The TTY guard is T15.9.

mod app;
mod view;

use std::io::{self, IsTerminal};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use crossterm::event::{self, Event, KeyEventKind};

use crate::config::Config;

/// `rtok tui`: alternate screen and raw mode until `q` / `Esc` / `Ctrl+C`. The terminal
/// is restored on every exit path — the loop's errors are returned, not panicked on, and
/// a panic still restores first: `try_init` installs ratatui's panic hook (0.30.2
/// `init.rs::set_panic_hook`), which calls `restore` before the previous hook unwinds.
pub fn run(cfg: Config) -> Result<()> {
    check_tty(io::stdin().is_terminal(), io::stdout().is_terminal())?;
    let mut terminal = ratatui::try_init().context("terminal init — is stdout a tty?")?;
    // `max(1)`: a zero cadence would busy-poll; `config validate` rejects it in the
    // file, this holds the line for `--tick-secs 0`.
    let tick = Duration::from_secs(cfg.tui.tick_secs.max(1));
    let mut app = app::App::background(&cfg);
    let res = event_loop(&mut terminal, &mut app, tick);
    ratatui::restore();
    res
}

/// How long the loop waits for a key before it checks for a finished background read.
const FRAME: Duration = Duration::from_millis(100);

/// Draw, then wait briefly for a key: a press updates the state, and every `tick` the
/// App re-reads the model through the config it holds — the same one-snapshot-per-tick
/// shape `rtok web`'s socket serves, from one owner (T15.4). Reads run off this thread,
/// so keys stay live while the model is slow.
fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut app::App,
    tick: Duration,
) -> Result<()> {
    let mut next = Instant::now() + tick;
    loop {
        terminal.draw(|frame| view::draw(frame, app))?;
        let wait = next.saturating_duration_since(Instant::now()).min(FRAME);
        if event::poll(wait)?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
            && app.key(key.code, key.modifiers)
        {
            return Ok(());
        }
        app.poll();
        if Instant::now() >= next {
            app.tick();
            next = Instant::now() + tick;
        }
    }
}

/// T15.9: both streams must be a tty — keys arrive on stdin, the screen is written to
/// stdout, and a pipe on either (CI, `rtok tui | less`, a daemon) would trap the caller
/// in raw mode or garble its pipe. Refused here, before `try_init`, no terminal state is
/// ever touched, so there is nothing to restore and no escape code reaches the pipe.
fn check_tty(stdin: bool, stdout: bool) -> Result<()> {
    if stdin && stdout {
        Ok(())
    } else {
        bail!("rtok tui needs a terminal on stdin and stdout; run it interactively, not piped")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T15.9: the guard decides on the two tty flags alone, so every combination is
    /// deterministic here — both streams, or refusal.
    #[test]
    fn tty_guard_demands_both_streams() {
        assert!(check_tty(true, true).is_ok());
        assert!(check_tty(true, false).is_err());
        assert!(check_tty(false, true).is_err());
        assert!(check_tty(false, false).is_err());
    }
}
