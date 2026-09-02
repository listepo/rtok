//! Re-read dedup (plan T4.4): same session/path/mode/range + sha256 → short hit.

use sha2::{Digest, Sha256};

use crate::plugin::{Ctx, Measurement, PostToolUse};
use crate::tokens::Class;

pub fn key(path: &str, mode: &str, range: Option<&str>) -> String {
    format!("{path}\t{mode}\t{}", range.unwrap_or(""))
}

pub fn hit(cx: &Ctx, key: &str, body: &[u8], lines: usize) -> Option<String> {
    let (Some(id), _) = cx.store.get_read_cache(&cx.session, key).ok().flatten()? else {
        return None;
    };
    let prev = cx.store.get_archive(&id).ok().flatten()?;
    if prev.as_slice() != body {
        return None;
    }
    let msg = format!("unchanged since {id} ({lines} lines)");
    let msg = if msg.len() < 80 {
        msg
    } else {
        format!("unchanged since {:.8} ({lines} lines)", id)
    };
    let _ = cx.record(&Measurement {
        plugin: "read",
        kind: "dedup",
        before_bytes: body.len() as u64,
        after_bytes: msg.len() as u64,
        est_before: cx.estimate(std::str::from_utf8(body).unwrap_or(""), Class::Code),
        est_after: cx.estimate(&msg, Class::Code),
        ref_id: Some(id),
        call_id: None,
    });
    Some(msg)
}

pub fn remember(cx: &Ctx, key: &str, body: &[u8]) -> anyhow::Result<String> {
    let id = cx
        .store
        .put_archive(&cx.session, body, &cx.config.core.archive_dir)?;
    cx.store
        .put_read_cache(&cx.session, key, &hex_sha256(body), Some(&id))?;
    Ok(id)
}

pub fn invalidate(ev: &PostToolUse<'_>, cx: &Ctx) {
    if ev.tool_name != "Edit" && ev.tool_name != "Write" {
        return;
    }
    let Some(path) = ev
        .tool_input
        .get("file_path")
        .or_else(|| ev.tool_input.get("path"))
        .and_then(|v| v.as_str())
    else {
        return;
    };
    let _ = cx.store.clear_read_cache(&cx.session, path);
}

fn hex_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::plugins::read::read;
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;

    fn cx(name: &str) -> (Ctx, PathBuf) {
        let dir = std::env::temp_dir().join(format!("rtok-dedup-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut c = Config::default();
        c.core.db_path = dir.join("rtok.db");
        c.core.archive_dir = dir.join("archive");
        c.plugins.read.allow_paths = vec![dir.clone()];
        (Ctx::open(c, name).unwrap(), dir)
    }

    #[test]
    fn two_identical_reads_second_is_short() {
        let (cx, dir) = cx("same");
        let p = dir.join("a.txt");
        fs::write(&p, "alpha\nbeta\n").unwrap();
        let path = p.to_str().unwrap();
        let first = read(&cx, path, "full", None).unwrap();
        assert!(first.contains("1:alpha"), "{first}");
        let second = read(&cx, path, "full", None).unwrap();
        assert!(second.len() < 80, "{second}");
        assert!(second.contains("unchanged"), "{second}");
        assert!(cx.store.measurement_count("read").unwrap() >= 1);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn edit_fixture_between_reads_is_full() {
        let (cx, dir) = cx("edit");
        let p = dir.join("a.txt");
        fs::write(&p, "alpha\n").unwrap();
        let path = p.to_str().unwrap();
        let _ = read(&cx, path, "full", None).unwrap();
        fs::write(&p, "omega\n").unwrap();
        let second = read(&cx, path, "full", None).unwrap();
        assert!(second.contains("1:omega"), "{second}");
        assert!(!second.contains("unchanged"), "{second}");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn post_tool_edit_clears_hit() {
        let (cx, dir) = cx("hook");
        let p = dir.join("a.txt");
        fs::write(&p, "alpha\n").unwrap();
        let path = p.to_str().unwrap();
        let _ = read(&cx, path, "full", None).unwrap();
        let input = json!({"file_path": path});
        invalidate(
            &PostToolUse {
                tool_name: "Edit",
                tool_input: &input,
                tool_response: &json!({}),
            },
            &cx,
        );
        let second = read(&cx, path, "full", None).unwrap();
        assert!(second.contains("1:alpha"), "{second}");
        let _ = fs::remove_dir_all(dir);
    }
}
