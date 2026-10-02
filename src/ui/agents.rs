// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! Strings for `rtok agents outdated` and `rtok agents update --check` (T279.1).

/// Shown when no host variant has rtok's plugin installed.
pub const NO_PLUGINS: &str = "no rtok plugins installed";

/// Every installed plugin is at or above the running rtok version (`installed`, `rtok` version).
pub fn all_current(installed: usize, rtok: &str) -> String {
    format!("all rtok plugins are up to date ({installed} installed, rtok {rtok})")
}

/// Hint after the outdated table (`hosts` is comma-separated host ids).
pub fn update_hint(hosts: &str) -> String {
    format!("run: rtok agents update {hosts}")
}
