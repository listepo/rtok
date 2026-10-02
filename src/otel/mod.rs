// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-only
// Licensed under GPL-3.0 only; see https://www.gnu.org/licenses/gpl-3.0.html

//! OpenTelemetry export (D19, P16): a projection of the ledgers, never a second recorder.
//! Design and the ledger → GenAI mapping table: `PLAN.md` in this directory.

pub mod export;
pub mod map;
pub mod metrics;
pub mod otlp;
