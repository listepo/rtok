// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-only
// Licensed under GPL-3.0 only; see https://www.gnu.org/licenses/gpl-3.0.html

use std::process::ExitCode;

fn main() -> ExitCode {
    if let Err(e) = rtok::cli::run() {
        let cfg = rtok::config::Config::load_lenient(None, None);
        rtok::log::append(&cfg, "error", "cli", "run", &format!("{e:#}"));
        // The bytes std's `Result` termination prints (`Error: {e:?}`), marked only on a terminal.
        eprintln!("{}", rtok::ui::style::error(&format!("Error: {e:?}")));
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
