//! T71.3: hub skill size limits (`skills/rtok/SKILL.md`).

use std::fs;
use std::path::PathBuf;

fn hub_skill() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skills/rtok/SKILL.md");
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
fn hub_skill_description_is_at_most_120_chars() {
    let content = hub_skill();
    let (front, _) = split_frontmatter(&content);
    let desc = description(front);
    assert!(
        desc.len() <= 120,
        "description is {} chars: {desc}",
        desc.len()
    );
}

#[test]
fn hub_skill_body_is_at_most_2kb_and_has_no_disable_model_invocation() {
    let content = hub_skill();
    let (front, body) = split_frontmatter(&content);
    assert!(
        body.len() <= 2048,
        "body is {} bytes (limit 2048)",
        body.len()
    );
    assert!(
        !front.contains("disable-model-invocation"),
        "hub skill must stay model-invokable"
    );
}
