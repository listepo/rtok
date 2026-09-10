//! `rtok report` (P22, decision D24): the operator model as one document.
//!
//! [`Document`] is the format-neutral shape — T22.2 (html), T22.3 (pdf) and T22.4 (`--ai`)
//! render these same sections, in this order; [`markdown`] is the first renderer because
//! it needs none, which is what proves the *content* before any layout work starts. The
//! document is assembled from the D23 model (`crate::web::model`) and nothing else: a
//! number that is not in a model struct cannot appear here, and every figure carries the
//! rows it came from and the window it covers.

pub mod advice;
pub mod ai;
pub mod html;
pub mod markdown;
pub mod pdf;

use std::path::Path;

use anyhow::Result;
use serde::Serialize;

use crate::config::Config;
use crate::web::model;

/// One recommendation: a rule over the ledgers that prints what triggered it and the rows
/// it read ([`advice`], T22.5; never a language-model call). Empty when no rule fires —
/// the section heading still renders, with a "no recommendations" line.
#[derive(Debug, Serialize)]
pub struct Recommendation {
    pub rule: String,
    pub finding: String,
    pub evidence: String,
}

/// The whole report: the fixed section set of P22. Ledger numbers come from
/// [`model::ReportLedgers`] (one store open), Config from [`model::config_entries`]
/// (the `config show --sources` page), Doctor from [`model::doctor`] (live probes).
#[derive(Debug)]
pub struct Document {
    pub ledgers: model::ReportLedgers,
    pub config: Vec<model::ConfigEntry>,
    /// The `rtok doctor` page — the one section whose figures are live probes rather than
    /// store rows, and the only one the report says that about.
    pub doctor: crate::doctor::Report,
    /// T22.5 rules over the ledgers, most recoverable tokens first; empty when nothing
    /// fires, and the section says so instead of filler advice.
    pub recommendations: Vec<Recommendation>,
}

/// Build the document from the model. `home` / `config_file` feed the Config page exactly
/// as `config show --sources` resolves them, so the two cannot disagree about a layer.
pub fn document(cfg: &Config, home: &Path, config_file: Option<&Path>) -> Result<Document> {
    let ledgers = model::report_ledgers(cfg)?;
    let recommendations = advice::recommendations(&ledgers, cfg);
    Ok(Document {
        ledgers,
        config: model::config_entries(home, config_file)?,
        doctor: model::doctor(cfg)?,
        recommendations,
    })
}
