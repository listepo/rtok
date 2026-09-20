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
    use super::*;
    use crate::agents::HOSTS;
    use crate::testutil::Vfs;
    use rtok_agent_sdk::{NO_CHANGES, OWNED_MARKER, SkillPlan, skill_plan};

    const HUB: &str = "src/SKILL.md";
    const DEST: &str = "skills/rtok";
    const DEST_MD: &str = "skills/rtok/SKILL.md";
    const FOREIGN: &str = "skills/other/SKILL.md";

    fn marker() -> String {
        format!("{DEST}/{OWNED_MARKER}")
    }

    fn dest_exists(vfs: &Vfs) -> bool {
        !vfs.paths_under(DEST).is_empty() || vfs.exists(DEST)
    }

    fn vfs_sync(vfs: &mut Vfs, remove: bool) -> String {
        match skill_plan(remove, dest_exists(vfs), vfs.exists(&marker())) {
            SkillPlan::NoChanges => NO_CHANGES.into(),
            SkillPlan::LeaveForeign => {
                format!("leave {DEST} (not an rtok skill; remove by hand)")
            }
            SkillPlan::Copy => {
                if let Some(body) = vfs.read(HUB).map(ToOwned::to_owned) {
                    vfs.write(DEST_MD, body);
                }
                vfs.write(marker(), b"");
                format!("+ skill → {DEST}")
            }
            SkillPlan::Remove => {
                let keep: Vec<(String, Vec<u8>)> = vfs
                    .paths()
                    .filter(|p| p != DEST && !p.starts_with("skills/rtok/"))
                    .filter_map(|p| vfs.read(&p).map(|b| (p, b.to_vec())))
                    .collect();
                *vfs = Vfs::new();
                for (p, b) in keep {
                    vfs.write(p, b);
                }
                format!("- skill {DEST}")
            }
        }
    }

    #[test]
    fn install_reinstall_remove_keeps_foreign() {
        let mut vfs = Vfs::new();
        vfs.write(HUB, "hub body\n");
        vfs.write(FOREIGN, "# other\n");

        let first = vfs_sync(&mut vfs, false);
        assert!(first.starts_with("+ skill"), "{first}");
        assert!(vfs.exists(&marker()));
        assert_eq!(vfs.read_str(DEST_MD), Some("hub body\n"));
        let before = vfs.read(DEST_MD).unwrap().to_vec();
        assert_eq!(vfs_sync(&mut vfs, false), NO_CHANGES);
        assert_eq!(vfs.read(DEST_MD), Some(before.as_slice()));
        assert!(vfs_sync(&mut vfs, true).starts_with("- skill"));
        assert_eq!(vfs_sync(&mut vfs, true), NO_CHANGES);
        assert_eq!(vfs.read_str(FOREIGN), Some("# other\n"));
        assert!(!vfs.exists(DEST_MD));
        assert!(!vfs.exists(&marker()));
    }

    #[test]
    fn leaves_a_foreign_skill_tree() {
        let mut vfs = Vfs::new();
        vfs.write(HUB, "hub body\n");
        vfs.write(DEST_MD, "# foreign\n");

        let out = vfs_sync(&mut vfs, false);
        assert!(out.contains("leave"), "{out}");
        assert_eq!(vfs.read_str(DEST_MD), Some("# foreign\n"));
        assert!(!vfs.exists(&marker()));
    }

    #[test]
    fn dest_maps_documented_roots_and_skips_the_rest() {
        let cfg = Config::default();
        for id in HOSTS {
            match *id {
                "claude" | "cursor" | "codex" | "opencode" | "copilot" => {
                    assert!(dest(id, &cfg).is_some(), "{id} has a §10.1 skill root");
                }
                _ => assert!(dest(id, &cfg).is_none(), "{id} has no skill format"),
            }
        }
        assert!(dest("gemini", &cfg).is_none(), "no gemini host installer");
        assert!(
            dest("claude", &cfg)
                .unwrap()
                .ends_with(".claude/skills/rtok")
        );
        assert!(
            dest("cursor", &cfg)
                .unwrap()
                .ends_with(".cursor/skills/rtok")
        );
        assert!(dest("codex", &cfg).unwrap().ends_with(".codex/skills/rtok"));
        assert!(
            dest("opencode", &cfg)
                .unwrap()
                .ends_with("opencode/skills/rtok")
        );
        assert!(
            dest("copilot", &cfg)
                .unwrap()
                .ends_with(".copilot/skills/rtok")
        );
    }
}
