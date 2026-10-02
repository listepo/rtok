// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! `rtok agents outdated` / `rtok agents update --check` (T279.1).

use anyhow::{Context, Result, bail};
use semver::Version;
use serde::Serialize;

use super::plugin_install::install_probe;
use super::plugin_version::{Receipt, is_behind, read_installed, receipt_path};
use super::{HOSTS, Kind, Support, host, wants};
use crate::config::Config;
use crate::render::{Col, table};
use crate::ui::agents as ui;

/// Exit code for `--exit-code` when at least one plugin is behind the running rtok.
pub const EXIT_OUTDATED: i32 = 10;

/// Host / variant selection — same flags as `agents update`.
#[derive(Debug, Clone, Default)]
pub struct OutdatedSelection {
    pub hosts: Vec<String>,
    pub cli: bool,
    pub desktop: bool,
    pub all: bool,
}

/// One outdated plugin row.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Outdated {
    pub agent: String,
    pub variant: String,
    pub installed: String,
    pub available: String,
    pub source: String,
    pub legacy: bool,
}

#[derive(Debug, Serialize)]
pub struct OutdatedReport {
    pub rtok: String,
    pub outdated: Vec<Outdated>,
    pub installed: usize,
}

/// List every selected host variant whose rtok plugin is older than the running binary.
pub fn outdated(cfg: &Config, selection: &OutdatedSelection) -> Result<Vec<Outdated>> {
    Ok(scan(cfg, selection)?.outdated)
}

pub fn report(cfg: &Config, selection: &OutdatedSelection) -> Result<OutdatedReport> {
    scan(cfg, selection)
}

fn scan(cfg: &Config, selection: &OutdatedSelection) -> Result<OutdatedReport> {
    let target = target_version()?;
    let receipt = Receipt::read(&receipt_path(cfg))?;
    let ids = host_ids(selection)?;
    let want = |kind: Kind| wants(kind, selection.cli, selection.desktop, selection.all);
    let mut out = Vec::new();
    let mut installed = 0usize;
    for id in ids {
        let Some(agent) = host(&id) else { continue };
        for v in agent.variants().iter().filter(|v| want(v.kind)) {
            if !agent.installed(cfg, v.kind).contains(&"plugin") {
                continue;
            }
            if matches!(agent.support(v.kind, "plugin"), Support::No(_)) {
                continue;
            }
            installed += 1;
            let probe = install_probe(&id, cfg, v.kind, &receipt).unwrap_or(
                super::plugin_install::InstallProbe {
                    version_file: std::path::PathBuf::from("\0"),
                    host_record_version: None,
                    fallback_source: super::plugin_version::Source::Github,
                },
            );
            if let Some(row) = installed_row(
                &id,
                v.kind,
                &probe.version_file,
                &receipt,
                probe.host_record_version.as_deref(),
                probe.fallback_source,
                &target,
            )? {
                out.push(row);
            }
        }
    }
    Ok(OutdatedReport {
        rtok: target.to_string(),
        outdated: out,
        installed,
    })
}

fn installed_row(
    agent: &str,
    kind: Kind,
    version_file: &std::path::Path,
    receipt: &Receipt,
    host_record: Option<&str>,
    fallback: super::plugin_version::Source,
    target: &Version,
) -> Result<Option<Outdated>> {
    let got = read_installed(version_file, receipt, agent, host_record, fallback)?;
    if !is_behind(&got.version, target) {
        return Ok(None);
    }
    let installed = if got.legacy {
        "legacy".to_string()
    } else {
        got.version.to_string()
    };
    Ok(Some(Outdated {
        agent: agent.to_string(),
        variant: kind.as_str().to_string(),
        installed,
        available: target.to_string(),
        source: got.source.to_string(),
        legacy: got.legacy,
    }))
}

fn host_ids(selection: &OutdatedSelection) -> Result<Vec<String>> {
    if selection.hosts.is_empty() {
        return Ok(HOSTS.iter().map(|s| (*s).to_string()).collect());
    }
    for h in &selection.hosts {
        if host(h).is_none() {
            bail!("unknown host: {h}");
        }
    }
    Ok(selection.hosts.clone())
}

fn target_version() -> Result<Version> {
    Version::parse(env!("CARGO_PKG_VERSION")).context("CARGO_PKG_VERSION")
}

/// Human output for a report; empty when `json` (caller prints JSON only).
pub fn print_human(rep: &OutdatedReport) -> String {
    if rep.installed == 0 {
        return format!("{}\n", ui::NO_PLUGINS);
    }
    if rep.outdated.is_empty() {
        return format!("{}\n", ui::all_current(rep.installed, &rep.rtok));
    }
    let cols = [Col::left(5), Col::left(8), Col::left(9), Col::left(6)];
    let mut rows: Vec<Vec<String>> = vec![vec![
        "agent".into(),
        "installed".into(),
        "available".into(),
        "source".into(),
    ]];
    rows.extend(rep.outdated.iter().map(|r| {
        vec![
            r.agent.clone(),
            r.installed.clone(),
            r.available.clone(),
            r.source.clone(),
        ]
    }));
    let hosts: Vec<String> = rep
        .outdated
        .iter()
        .map(|r| r.agent.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    format!(
        "{}\n{}\n",
        table(&cols, &rows),
        ui::update_hint(&hosts.join(","))
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    fn target() -> Version {
        Version::parse(env!("CARGO_PKG_VERSION")).unwrap()
    }

    #[test]
    fn is_behind_ignores_build_metadata_on_equal_base() {
        let t = target();
        let local = super::super::plugin_version::local_version(&t, "12c7e91");
        assert!(!is_behind(&local, &t));
    }

    #[test]
    fn is_behind_flags_older_patch() {
        let t = target();
        let old = Version::parse("0.0.1").unwrap();
        assert!(is_behind(&old, &t));
    }

    #[test]
    fn newer_installed_is_not_behind() {
        let t = target();
        let mut newer = t.clone();
        newer.patch += 1;
        assert!(!is_behind(&newer, &t));
        assert_eq!(newer.cmp_precedence(&t), Ordering::Greater);
    }
}
