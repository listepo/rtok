//! D23's gate: `rtok web` and `rtok tui` are two renderings of one operator model, so
//! a page that exists on one surface and not the other is a defect. Each surface
//! contributes its own set here and the assert fails naming the page that drifted;
//! when `rtok tui` lands (T15.1+) its tabs join the same compare.

use rtok::config::Config;
use rtok::web::frame;
use rtok::web::model;

fn config() -> Config {
    let dir = std::env::temp_dir().join(format!("rtok-parity-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    Config::load_from(&dir).expect("config")
}

/// The pages the model offers, by name.
fn model_pages() -> Vec<String> {
    let mut names: Vec<String> = model::pages()
        .iter()
        .map(|(page, _)| page.to_string())
        .collect();
    names.sort();
    names
}

/// The pages the web surface serves: the keys of the frame `/ws` sends, envelope
/// dropped and each key read back to its page name. A key with no entry in the
/// model's table is its own page, so a web-only page names itself in the failure.
fn web_pages(cfg: &Config) -> Vec<String> {
    let v: serde_json::Value = serde_json::from_str(&frame(cfg)).expect("frame is json");
    let mut names: Vec<String> = v
        .as_object()
        .expect("frame is an object")
        .keys()
        .filter(|key| *key != "type") // the wire envelope, not a page
        .map(|key| {
            model::pages()
                .iter()
                .find(|(_, wire_key)| *wire_key == key.as_str())
                .map_or_else(|| key.clone(), |(page, _)| page.to_string())
        })
        .collect();
    names.sort();
    names
}

#[test]
fn web_serves_exactly_the_pages_the_model_offers() {
    let cfg = config();
    assert_eq!(
        model_pages(),
        web_pages(&cfg),
        "a page exists on one surface and not the other (D23)"
    );
}
