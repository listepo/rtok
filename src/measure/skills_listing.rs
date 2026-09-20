//! T71.4: skill-listing overhead in proxied `/v1/messages` request bodies.

use serde_json::Value;

/// One measured skills block inside a captured proxy request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillsListingMeasure {
    /// Bytes of the whole `<available_skills>` block (tags included).
    pub block_bytes: usize,
    pub count: usize,
    /// Name, path, and wrapper bytes beyond each skill's description text.
    pub framing_bytes_per_skill: usize,
}

/// Parse a proxied Anthropic Messages request and measure the skills listing block.
pub fn measure(request: &[u8]) -> Option<SkillsListingMeasure> {
    let root: Value = serde_json::from_slice(request).ok()?;
    let mut best: Option<SkillsListingMeasure> = None;
    for text in system_texts(&root) {
        if let Some(m) = measure_block(text) {
            best = Some(match best {
                None => m,
                Some(prev) if m.block_bytes > prev.block_bytes => m,
                Some(prev) => prev,
            });
        }
    }
    best
}

fn system_texts(root: &Value) -> Vec<&str> {
    match root.get("system") {
        Some(Value::String(s)) => vec![s.as_str()],
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter_map(|b| {
                b.get("text")
                    .and_then(|t| t.as_str())
                    .or_else(|| b.as_str())
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn measure_block(text: &str) -> Option<SkillsListingMeasure> {
    const START: &str = "<available_skills>";
    const END: &str = "</available_skills>";
    let start = text.find(START)?;
    let end = text[start..].find(END)?;
    let block = &text[start..start + end + END.len()];
    let entries = skill_entries(block);
    if entries.is_empty() {
        return None;
    }
    let framing = entries
        .iter()
        .map(|(entry, desc)| entry.len().saturating_sub(desc.len()))
        .sum::<usize>()
        / entries.len();
    Some(SkillsListingMeasure {
        block_bytes: block.len(),
        count: entries.len(),
        framing_bytes_per_skill: framing,
    })
}

/// `(entry_bytes, description_bytes)` per listed skill.
fn skill_entries(block: &str) -> Vec<(&str, &str)> {
    const OPEN: &str = "<agent_skill";
    const CLOSE: &str = "</agent_skill>";
    let mut out = Vec::new();
    let mut rest = block;
    while let Some(i) = rest.find(OPEN) {
        rest = &rest[i..];
        let Some(close) = rest.find(CLOSE) else {
            break;
        };
        let entry = &rest[..close + CLOSE.len()];
        let Some(desc_start) = entry.find('>').map(|j| j + 1) else {
            rest = &rest[close + CLOSE.len()..];
            continue;
        };
        let Some(desc_end) = entry.rfind('<') else {
            rest = &rest[close + CLOSE.len()..];
            continue;
        };
        if desc_start < desc_end {
            out.push((entry, &entry[desc_start..desc_end]));
        }
        rest = &rest[close + CLOSE.len()..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_available_skills_block() {
        let body = br#"{"model":"claude","messages":[],"system":[{"type":"text","text":"prefix\n<available_skills>\n<agent_skill fullPath=\"/a/x\">alpha skill</agent_skill>\n<agent_skill fullPath=\"/b/y\">beta has a longer description here</agent_skill>\n</available_skills>\nsuffix"}]}"#;
        let m = measure(body).expect("skills block");
        assert_eq!(m.count, 2);
        assert!(m.block_bytes > 80);
        assert!(m.framing_bytes_per_skill > 20);
        assert!(m.framing_bytes_per_skill < 120);
    }
}
