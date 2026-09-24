//! `rtok graph status` — index health for one root (T68.3, T60.1).

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;

use rtok_plugin_sdk::Ctx;

use super::index;
use super::pending_paths;
use crate::config::Config;
use crate::plugin::Runtime;

#[derive(Debug, Clone, Serialize)]
pub struct GraphStatus {
    pub root: String,
    pub rows: i64,
    pub files: i64,
    pub pending: Vec<String>,
    pub watch: String,
    pub indexed_at: Option<i64>,
}

pub fn collect(cx: &Ctx, root: &Path) -> Result<GraphStatus> {
    let key = index::canon(root);
    Ok(GraphStatus {
        root: key.clone(),
        rows: cx.symbol_count(&key)?,
        files: cx.symbol_file_count(&key)?,
        pending: pending_paths(cx, root)?,
        watch: cx
            .plugin_config::<crate::config::Graph>("graph")
            .watch
            .clone(),
        indexed_at: cx.symbol_indexed_at(&key)?,
    })
}

pub fn run(cfg: &Config, path: Option<PathBuf>, json: bool) -> Result<()> {
    let cx = Runtime::open(cfg.clone(), "graph-status")?;
    let root = path.unwrap_or(std::env::current_dir()?);
    let status = collect(&Ctx::new(&cx), &root)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else {
        print!("{}", format_table(&status));
    }
    Ok(())
}

/// Rendered for `rtok graph status` and the Graph page (T230) — `pub(crate)` so
/// `web::model` reuses the same text instead of re-formatting `GraphStatus` itself.
pub(crate) fn format_table(s: &GraphStatus) -> String {
    let mut out = String::new();
    out.push_str(&format!("root {}\n", s.root));
    out.push_str(&format!("rows {}\n", s.rows));
    out.push_str(&format!("files {}\n", s.files));
    out.push_str(&format!("pending {}\n", s.pending.len()));
    for p in &s.pending {
        out.push_str(&format!("  {p}\n"));
    }
    out.push_str(&format!("watch {}\n", s.watch));
    match s.indexed_at {
        Some(ts) => out.push_str(&format!("indexed_at {ts}\n")),
        None => out.push_str("indexed_at -\n"),
    }
    out
}
