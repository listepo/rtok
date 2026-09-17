//! T62.1: digest an oversized skill body before Claude Code injects it.
//!
//! `PreToolUse(Skill)` fires before the host appends `SKILL.md` to the context and may
//! deny with a reason the model reads. Over the cap the body is archived and the reason
//! is its markdown map plus the `expand` pointer: lossless, off by default, and every
//! other outcome (small body, host frontmatter keys, unknown path, read error) falls open.

use rtok_plugin_sdk::{Class, Ctx, Measurement, PreToolDecision, PreToolUse};
use std::path::{Path, PathBuf};

/// Frontmatter keys the host applies on invocation (Claude Code docs, 2026-09-17); a
/// denied skill would lose them, so such skills always load whole.
const HOST_KEYS: [&str; 4] = ["allowed-tools", "model", "context", "agent"];

pub(super) fn digest(ev: &PreToolUse, cx: &Ctx) -> Option<PreToolDecision> {
    let g = cx.plugin_config::<crate::config::Guard>("guard");
    if !g.skills {
        return None;
    }
    let name = ev.tool_input.get("skill")?.as_str()?.trim();
    let home = crate::config::env_user_home()?;
    let path = resolve(name, cx.cwd().map(Path::new), &home)?;
    let body = std::fs::read_to_string(path).ok()?;
    decide(cx, name, &body, u64::from(g.skill_max_bytes))
}

/// `SKILL.md` for a skill name: `plugin:skill` → that plugin's `installPath` from
/// `installed_plugins.json`; a bare name → project `.claude/skills`, then the user one.
/// A name carrying a path separator or `..` is not a skill name: fail open.
fn resolve(name: &str, cwd: Option<&Path>, home: &Path) -> Option<PathBuf> {
    if name.is_empty() || name.contains(['/', '\\']) || name.contains("..") {
        return None;
    }
    if let Some((plugin, skill)) = name.split_once(':') {
        let manifest = home
            .join(".claude")
            .join("plugins")
            .join("installed_plugins.json");
        let root = plugin_root(&manifest, plugin)?;
        return Some(root.join("skills").join(skill).join("SKILL.md"));
    }
    let project = cwd.map(|c| c.join(".claude").join("skills").join(name).join("SKILL.md"));
    match project {
        Some(p) if p.is_file() => Some(p),
        _ => Some(crate::doctor::skill_md_path(home, name)),
    }
}

/// `plugins."<plugin>@<marketplace>"[0].installPath` — Claude Code `installed_plugins.json`
/// version 2, checked on this machine 2026-09-18.
fn plugin_root(manifest: &Path, plugin: &str) -> Option<PathBuf> {
    let text = std::fs::read_to_string(manifest).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let entry = v
        .get("plugins")?
        .as_object()?
        .iter()
        .find(|(k, _)| k.split('@').next() == Some(plugin))?
        .1;
    Some(PathBuf::from(entry.get(0)?.get("installPath")?.as_str()?))
}

fn decide(cx: &Ctx, name: &str, body: &str, cap: u64) -> Option<PreToolDecision> {
    let bytes = body.len() as u64;
    if bytes <= cap || host_keys(body) {
        return None;
    }
    let id = cx.put_archive(body.as_bytes()).ok()?;
    let lines = body.lines().count();
    // The whole reason stays within the cap it enforces: map + intro + trailer.
    let map = truncate(outline(body), (cap as usize).saturating_sub(512));
    let reason = format!(
        "rtok kept the map of skill {name} ({} KB); the full body is archived, not loaded:\n{map}\
         [rtok {id} · {lines} lines · expand: rtok expand {id}]\n\
         Pull one section with `rtok expand {id} --grep <heading>`; set [plugins.guard] skills = false to load skills whole.",
        bytes / 1024
    );
    let _ = cx.record(&Measurement {
        plugin: "guard",
        kind: "skill",
        before_bytes: bytes,
        after_bytes: reason.len() as u64,
        est_before: cx.estimate(body, Class::Prose),
        est_after: cx.estimate(&reason, Class::Prose),
        ref_id: Some(id),
        call_id: None,
    });
    Some(PreToolDecision::Deny { reason })
}

/// True when the YAML frontmatter names a key the host applies at invocation.
fn host_keys(body: &str) -> bool {
    let Some(rest) = body.strip_prefix("---") else {
        return false;
    };
    let end = rest.find("\n---").unwrap_or(rest.len());
    rest[..end].lines().any(|l| {
        HOST_KEYS.iter().any(|k| {
            l.strip_prefix(k)
                .is_some_and(|r| r.trim_start().starts_with(':'))
        })
    })
}

/// Every heading with the first non-empty line under it, fenced blocks skipped. The
/// `read` plugin outlines code with tree-sitter only, so this is the one markdown
/// outliner in the tree (T62.1 close-out).
fn outline(body: &str) -> String {
    let mut out = String::new();
    let (mut fenced, mut want_line) = (false, false);
    for line in body.lines() {
        let t = line.trim();
        if t.starts_with("```") {
            fenced = !fenced;
        } else if fenced {
        } else if t.starts_with('#') {
            out.push_str(t);
            out.push('\n');
            want_line = true;
        } else if want_line && !t.is_empty() {
            out.push_str("  ");
            out.push_str(t);
            out.push('\n');
            want_line = false;
        }
    }
    out
}

/// Cut at a char boundary with an ellipsis.
fn truncate(mut s: String, cap: usize) -> String {
    if s.len() > cap {
        let mut end = cap;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
        s.push_str("…\n");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtok_plugin_sdk::Archive;

    fn body(sections: usize) -> String {
        let mut s = "---\nname: big\ndescription: many sections\n---\n".to_string();
        for i in 0..sections {
            s += &format!("## Section {i}\nFirst line of {i}.\n```sh\n# not a heading\n```\n");
        }
        s
    }

    #[test]
    fn small_bodies_and_host_keys_pass() {
        let cx = crate::testutil::runtime("skill-pass").0;
        let cx = Ctx::new(&cx);
        assert!(decide(&cx, "s", "# T\n\nthree\nlines\n", 8192).is_none());
        let keyed = format!("---\nallowed-tools: Bash\n---\n{}", body(300));
        assert!(keyed.len() > 8192);
        assert!(decide(&cx, "s", &keyed, 8192).is_none());
        assert!(!host_keys(&body(1)));
        assert!(host_keys("---\nmodel:  haiku\n---\n# x"));
    }

    #[test]
    fn oversized_body_is_denied_with_map_and_pointer_under_budget() {
        let rt = crate::testutil::runtime("skill-deny").0;
        let cx = Ctx::new(&rt);
        // ≈ 3,000 lines / ≈ 230 KB: the size the measured outlier had (research §10.2).
        let big = body(600) + &"x".repeat(200_000);
        let t0 = std::time::Instant::now();
        let d = decide(&cx, "big", &big, 8192);
        let ms = t0.elapsed().as_millis();
        let Some(PreToolDecision::Deny { reason }) = d else {
            panic!("{d:?}")
        };
        assert!(
            reason.starts_with("rtok kept the map of skill big ("),
            "{reason}"
        );
        assert!(
            reason.contains("## Section 0\n  First line of 0.\n"),
            "{reason}"
        );
        assert!(!reason.contains("not a heading"), "{reason}");
        assert!(reason.len() <= 8192, "{}", reason.len());
        let id = reason
            .split("expand: rtok expand ")
            .nth(1)
            .and_then(|r| r.split(']').next())
            .unwrap();
        assert!(
            reason.contains(&format!("[rtok {id} · {} lines ·", big.lines().count())),
            "{reason}"
        );
        assert_eq!(rt.get_archive(id).unwrap().unwrap(), big.as_bytes());
        let budget = if cfg!(debug_assertions) { 100 } else { 10 };
        assert!(ms < budget, "{ms} ms");
    }

    #[test]
    fn resolve_prefers_project_then_user_and_reads_the_plugin_manifest() {
        let home = crate::testutil::tmp_dir("skill-resolve");
        let proj = home.join("proj");
        let p = proj.join(".claude/skills/here/SKILL.md");
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, "# here").unwrap();
        std::fs::create_dir_all(home.join(".claude/plugins")).unwrap();
        std::fs::write(
            home.join(".claude/plugins/installed_plugins.json"),
            r#"{"version":2,"plugins":{"pony@market":[{"installPath":"/cache/pony/1.0"}]}}"#,
        )
        .unwrap();
        assert_eq!(resolve("here", Some(&proj), &home), Some(p));
        assert_eq!(
            resolve("there", Some(&proj), &home),
            Some(crate::doctor::skill_md_path(&home, "there"))
        );
        assert_eq!(
            resolve("pony:tail", None, &home),
            Some(PathBuf::from("/cache/pony/1.0/skills/tail/SKILL.md"))
        );
        assert_eq!(resolve("nope:tail", None, &home), None);
        assert_eq!(resolve("../etc", None, &home), None);
        assert_eq!(resolve("a/b", None, &home), None);
    }
}
