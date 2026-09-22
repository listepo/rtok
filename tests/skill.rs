//! T71.3 / T155: size limits for every shipped skill (`skills/<name>/SKILL.md`), and the
//! `worktrees` skill names only commands `rtok worktree` really has.

mod common;

use clap::CommandFactory;
use common::agents::{rtok, tmp, write_cfg};
use rtok::agents::skill::SKILLS;
use rtok::cli::Cli;
use std::fs;
use std::path::PathBuf;

fn hub_skill(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("skills/{name}/SKILL.md"));
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn split_frontmatter(content: &str) -> (&str, &str) {
    let content = content.strip_prefix("---\n").expect("opening ---");
    let (front, body) = content.split_once("\n---\n").expect("closing ---");
    (front, body)
}

fn description(front: &str) -> String {
    front
        .lines()
        .find_map(|l| l.strip_prefix("description: "))
        .expect("description:")
        .to_string()
}

#[test]
fn every_skill_description_is_at_most_120_chars() {
    for name in SKILLS {
        let content = hub_skill(name);
        let (front, _) = split_frontmatter(&content);
        let desc = description(front);
        assert!(
            desc.chars().count() <= 120,
            "{name}: description is {} chars: {desc}",
            desc.chars().count()
        );
        assert!(
            front.lines().any(|l| l == format!("name: {name}")),
            "{name}: frontmatter name must match the directory"
        );
    }
}

#[test]
fn every_skill_body_is_at_most_2kb_and_has_no_disable_model_invocation() {
    for name in SKILLS {
        let content = hub_skill(name);
        let (front, body) = split_frontmatter(&content);
        assert!(
            body.len() <= 2048,
            "{name}: body is {} bytes (limit 2048)",
            body.len()
        );
        assert!(
            !front.contains("disable-model-invocation"),
            "{name}: hub skill must stay model-invokable"
        );
    }
}

/// T155: every inline `rtok worktree <sub> … --flag …` span in the skill resolves against the
/// real CLI, so the skill cannot teach a command or flag that `rtok worktree --help` lacks.
#[test]
fn worktrees_skill_names_only_commands_rtok_worktree_has() {
    let cli = Cli::command();
    let worktree = cli.find_subcommand("worktree").expect("rtok worktree");
    let content = hub_skill("worktrees");
    let (_, body) = split_frontmatter(&content);
    let mut seen = Vec::new();
    for span in body.split('`').skip(1).step_by(2) {
        let Some(rest) = span.strip_prefix("rtok worktree ") else {
            continue;
        };
        let mut words = rest.split_whitespace();
        let sub = words.next().expect("subcommand after `rtok worktree`");
        let cmd = worktree
            .find_subcommand(sub)
            .unwrap_or_else(|| panic!("`rtok worktree {sub}` is not a subcommand"));
        for flag in words.filter_map(|w| w.strip_prefix("--")) {
            assert!(
                cmd.get_arguments().any(|a| a.get_long() == Some(flag)),
                "`rtok worktree {sub}` has no `--{flag}`"
            );
        }
        seen.push(sub);
    }
    for sub in ["add", "list", "gc", "clean"] {
        assert!(
            seen.contains(&sub),
            "the skill must show `rtok worktree {sub}`"
        );
    }
}

/// T155: one install copies every shipped skill under the host's skill root and one remove
/// takes them all away (codex: no plugin offer, so no flags).
#[test]
fn install_copies_every_skill_and_remove_takes_them_away() {
    let home = tmp("skills-e2e");
    let cfg = write_cfg(&home);
    let root = home.join(".codex/skills");
    let out = rtok(&["agents", "install", "codex"], &cfg, &home);
    for name in SKILLS {
        assert!(root.join(name).join("SKILL.md").is_file(), "{name}: {out}");
        assert!(out.contains(&format!("skills/{name}")), "{name}: {out}");
    }
    let out = rtok(&["agents", "remove", "codex"], &cfg, &home);
    for name in SKILLS {
        assert!(!root.join(name).exists(), "{name}: {out}");
    }
}
