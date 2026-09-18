//! Managed memory block sync into CLAUDE.md / AGENTS.md (plan T69.6).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use rtok_agent_sdk::backup;

pub const START: &str = "<!-- rtok:memory -->";
pub const END: &str = "<!-- /rtok:memory -->";

pub struct Options {
    pub file: PathBuf,
    pub budget: u32,
    pub dry_run: bool,
    pub remove: bool,
    pub force: bool,
}

pub fn run(cfg: &crate::config::Config, opts: &Options) -> Result<String> {
    let rt = crate::plugin::Runtime::open(cfg.clone(), "memory-sync")?;
    let path = &opts.file;
    if opts.remove {
        return remove_block(&rt, path, opts.dry_run);
    }
    let block = render_block(&rt, opts.budget)?;
    let mut text = std::fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?;
    if let Some((start, end)) = find_block(&text) {
        let old = &text[start..end];
        if let Some(hash) = rt.store.memory_sync_hash(path)? {
            let cur = crate::store::hex_sha256(old.as_bytes());
            if hash != cur && !opts.force {
                bail!(
                    "hand-edited managed block in {}; use --force or --remove",
                    path.display()
                );
            }
        }
        text.replace_range(start..end, &block);
    } else {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&block);
    }
    if opts.dry_run {
        return Ok(format!("would write {} bytes to {}", block.len(), path.display()));
    }
    backup(path)?;
    std::fs::write(path, &text).with_context(|| format!("write {}", path.display()))?;
    rt.store
        .put_memory_sync_hash(path, &crate::store::hex_sha256(block.as_bytes()))?;
    Ok(format!("synced {} ({} bytes)", path.display(), block.len()))
}

fn render_block(rt: &crate::plugin::Runtime, budget: u32) -> Result<String> {
    let cap = budget.max(1);
    let rows = rt.store.list_note_titles(None, cap * 4)?;
    let mut lines = vec![START.to_string()];
    for (id, title) in rows {
        lines.push(format!("{id} {title}"));
        let body = lines.join("\n") + "\n" + END;
        if rt.estimate(&body, rtok_plugin_sdk::Class::Prose) > cap {
            lines.pop();
            break;
        }
    }
    lines.push(END.to_string());
    Ok(lines.join("\n") + "\n")
}

fn find_block(text: &str) -> Option<(usize, usize)> {
    let start = text.find(START)?;
    let end = text[start..].find(END)? + start + END.len();
    Some((start, end))
}

fn remove_block(rt: &crate::plugin::Runtime, path: &Path, dry: bool) -> Result<String> {
    let mut text = std::fs::read_to_string(path)?;
    let Some((start, end)) = find_block(&text) else {
        return Ok(format!("no managed block in {}", path.display()));
    };
    text.replace_range(start..end, "");
    if dry {
        return Ok(format!("would remove block from {}", path.display()));
    }
    backup(path)?;
    std::fs::write(path, text)?;
    let _ = rt.store.delete_memory_sync_hash(path)?;
    Ok(format!("removed managed block from {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_block_and_refuses_hand_edit() {
        let (mut cfg, dir) = crate::testutil::config("sync");
        let path = dir.join("AGENTS.md");
        std::fs::write(&path, b"# rules\n").unwrap();
        cfg.core.db_path = dir.join("rtok.db");
        let rt = crate::plugin::Runtime::open(cfg.clone(), "sync").unwrap();
        crate::plugins::memory::mem_save(&rt, "note", "alpha", "body", None).unwrap();
        let opts = Options {
            file: path.clone(),
            budget: 300,
            dry_run: false,
            remove: false,
            force: false,
        };
        run(&cfg, &opts).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(START));
        assert!(text.starts_with("# rules\n"));
        std::fs::write(&path, text.replace("alpha", "beta")).unwrap();
        assert!(run(&cfg, &opts).is_err());
    }
}
