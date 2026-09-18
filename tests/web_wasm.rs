//! T60.7: WASM bundle size gate for `rtok web`.

use std::path::PathBuf;

/// Upper bound from the optimized build measured in `research.md` (T60.7, 2026-09-18).
const WASM_SIZE_GATE: u64 = 4_500_000;

fn wasm_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("crates/rtok-webui/pkg/rtok_webui_bg.wasm")
}

#[test]
fn wasm_bundle_is_under_the_measured_gate() {
    let path = wasm_path();
    if !path.is_file() {
        eprintln!(
            "skip: {} missing — run `just web` or wasm-pack build crates/rtok-webui",
            path.display()
        );
        return;
    }
    let bytes = path.metadata().expect("wasm metadata").len();
    assert!(
        bytes <= WASM_SIZE_GATE,
        "rtok_webui_bg.wasm is {bytes} bytes, gate is {WASM_SIZE_GATE}"
    );
}
