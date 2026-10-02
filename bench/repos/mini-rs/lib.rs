// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

pub fn sink() {}

pub fn alpha() {
    sink();
}

pub fn beta() {
    sink();
}

pub fn gamma() {
    alpha();
}
