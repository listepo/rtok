//! T71.3: install `skills/rtok/` into each host's documented skill root (`research.md` §10.1).

use std::path::PathBuf;

use anyhow::Result;
use rtok_agent_sdk::{NO_CHANGES, SkillCopy};

use crate::config::Config;

use super::{apply, home_dir, skill_src};

/// User skill root for a host that documents the Agent Skills format; `None` = untouched.
pub fn dest(host: &str, cfg: &Config) -> Option<PathBuf> {
    let home = home_dir();
    match host {
        "claude" => Some(home.join(".claude/skills/rtok")),
        "cursor" => Some(home.join(".cursor/skills/rtok")),
        "codex" => Some(home.join(".codex/skills/rtok")),
        "opencode" => cfg
            .setup
            .opencode
            .config_path
            .parent()
            .map(|p| p.join("skills/rtok")),
        "copilot" => Some(cfg.setup.copilot.dir.join("skills/rtok")),
        _ => None,
    }
}

fn label(host: &str) -> Option<&'static str> {
    match host {
        "claude" => Some("~/.claude/skills/rtok"),
        "cursor" => Some("~/.cursor/skills/rtok"),
        "codex" => Some("~/.codex/skills/rtok"),
        "opencode" => Some("~/.config/opencode/skills/rtok"),
        "copilot" => Some("~/.copilot/skills/rtok"),
        _ => None,
    }
}

/// Copy or remove the hub skill for `host` (one report line).
pub fn sync(host: &str, cfg: &Config, remove: bool) -> Result<String> {
    let Some(dest) = dest(host, cfg) else {
        return Ok(NO_CHANGES.into());
    };
    SkillCopy {
        src: skill_src(),
        dest,
        label: label(host),
    }
    .run(&apply(cfg), remove)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use rtok_agent_sdk::{Apply, NO_CHANGES, OWNED_MARKER, SkillCopy};

    fn apply() -> Apply {
        Apply {
            dry_run: false,
            backup: false,
            yes: false,
        }
    }

    fn fixture() -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
        let root = crate::testutil::tmp_dir("skill");
        let src = root.join("src");
        let skills = root.join("skills");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("SKILL.md"), "hub body\n").unwrap();
        (root, src, skills)
    }

    #[test]
    fn install_reinstall_remove_keeps_foreign() {
        let (root, src, skills) = fixture();
        let dest = skills.join("rtok");
        let foreign = skills.join("other");
        fs::create_dir_all(&foreign).unwrap();
        fs::write(foreign.join("SKILL.md"), "# other\n").unwrap();

        let copy = SkillCopy {
            src,
            dest: dest.clone(),
            label: None,
        };
        let first = copy.run(&apply(), false).unwrap();
        assert!(first.starts_with("+ skill"), "{first}");
        assert!(dest.join(OWNED_MARKER).is_file());
        assert_eq!(copy.run(&apply(), false).unwrap(), NO_CHANGES);
        assert_eq!(copy.run(&apply(), true).unwrap(), format!("- skill {}", dest.display()));
        assert_eq!(copy.run(&apply(), true).unwrap(), NO_CHANGES);
        assert!(foreign.join("SKILL.md").is_file());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn leaves_a_foreign_skill_tree() {
        let (root, src, skills) = fixture();
        let dest = skills.join("rtok");
        fs::create_dir_all(&dest).unwrap();
        fs::write(dest.join("SKILL.md"), "# foreign\n").unwrap();

        let out = SkillCopy {
            src,
            dest: dest.clone(),
            label: None,
        }
        .run(&apply(), false)
        .unwrap();
        assert!(out.contains("leave"), "{out}");
        assert_eq!(fs::read_to_string(dest.join("SKILL.md")).unwrap(), "# foreign\n");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn dest_maps_documented_roots() {
        let cfg = Config::default();
        assert!(dest("claude", &cfg).unwrap().ends_with(".claude/skills/rtok"));
        assert!(dest("cursor", &cfg).unwrap().ends_with(".cursor/skills/rtok"));
        assert!(dest("codex", &cfg).unwrap().ends_with(".codex/skills/rtok"));
        assert!(dest("opencode", &cfg)
            .unwrap()
            .ends_with("opencode/skills/rtok"));
        assert!(dest("copilot", &cfg).unwrap().ends_with(".copilot/skills/rtok"));
        assert!(dest("pi", &cfg).is_none());
    }
}
