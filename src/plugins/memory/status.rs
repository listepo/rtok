//! `rtok memory status` — renders [`crate::web::model::MemoryStatus`] (T69.4).

use anyhow::Result;
use std::fmt::Write as _;

use crate::config::Config;
use crate::web::model::{self, MemoryStatus};

pub fn run(cfg: &Config, project: Option<&str>, since: Option<&str>, json: bool) -> Result<()> {
    let status = model::memory_status(cfg, project, since)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else {
        print!("{}", format_table(&status));
    }
    Ok(())
}

fn format_table(s: &MemoryStatus) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "since {}", s.since);
    if let Some(p) = &s.project {
        let _ = writeln!(out, "project {p}");
    }
    let _ = writeln!(
        out,
        "notes  live {}  pinned {}  retired {}  body {} B",
        s.notes.live, s.notes.pinned, s.notes.retired, s.notes.body_bytes
    );
    let _ = writeln!(
        out,
        "recall  count {}  stood_for {} B  injected {} B",
        s.recall.recalls, s.recall.stood_for_bytes, s.recall.injected_bytes
    );
    let _ = writeln!(
        out,
        "calls  mem_search {}  mem_get {}",
        s.calls.mem_search, s.calls.mem_get
    );
    for block in &s.by_project {
        let name = block.project.as_deref().unwrap_or("-");
        for kind in &block.kinds {
            let _ = writeln!(
                out,
                "project {name}  kind {}  live {}  pinned {}  retired {}  body {} B  ts {}..{}",
                kind.kind,
                kind.live,
                kind.pinned,
                kind.retired,
                kind.body_bytes,
                kind.oldest_ts,
                kind.newest_ts
            );
        }
    }
    out
}
