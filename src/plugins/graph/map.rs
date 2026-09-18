//! T52.3 — ranked repo map at SessionStart (off when `map_tokens = 0`).

use rtok_plugin_sdk::{Class, Ctx, Injection, SessionStart};

use super::index;

pub fn session_start(_ev: &SessionStart, cx: &Ctx) -> Option<Injection> {
    let cfg = cx.plugin_config::<crate::config::Graph>("graph");
    if cfg.map_tokens == 0 {
        return None;
    }
    let cwd = cx
        .cwd()
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::current_dir().ok())?;
    let rows = cx.symbol_top_refs(&index::canon(&cwd), i64::from(cfg.map_tokens)).ok()?;
    if rows.is_empty() {
        return None;
    }
    let mut lines = vec!["repo map".to_string()];
    for (name, refs, path, line, _kind) in &rows {
        lines.push(format!("{name} {path}:{line} {refs}"));
    }
    let mut text = lines.join("
");
    while cx.estimate(&text, Class::Code) > cfg.map_tokens && lines.len() > 1 {
        lines.pop();
        text = lines.join("
");
    }
    if lines.len() == 1 {
        return None;
    }
    Some(Injection {
        plugin: "graph",
        text,
        priority: 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::Ctx;
    use rtok_plugin_sdk::SessionStart;

    #[test]
    fn off_by_default() {
        let (rt, _dir) = crate::testutil::runtime("map-off");
        let ctx = Ctx::new(&rt);
        assert!(session_start(&SessionStart { source: "startup" }, &ctx).is_none());
    }
}
