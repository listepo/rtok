//! T81/T111: the web UI ships inside the release binary, and the places that say so
//! stay in step. Pure file reads — the release itself is built in CI, but a dropped
//! embed guard, a hand-edited `release.yml` or a drifted size gate is caught here.

use std::path::{Path, PathBuf};

fn repo(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)
}

fn read(rel: &str) -> String {
    let path = repo(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// T111: a ketch install keeps only the executable, so the bundle is compiled in.
/// The release job must demand it — otherwise `build.rs` quietly embeds nothing.
#[test]
fn the_release_build_requires_the_embedded_bundle() {
    let build = read("build.rs");
    assert!(
        build.contains(r#"Ok("require")"#) && build.contains("rtok_web_embed"),
        "build.rs no longer embeds the bundle or no longer honours RTOK_WEB_EMBED=require"
    );
    for rel in [".github/build-setup.yml", ".github/workflows/release.yml"] {
        assert!(
            read(rel).contains("RTOK_WEB_EMBED=require"),
            "{rel} no longer exports RTOK_WEB_EMBED=require before dist build"
        );
    }
    let manifest = read("Cargo.toml");
    assert!(
        !manifest.contains(r#""crates/rtok-webui/pkg/""#),
        "the archive would carry a second copy of the embedded bundle"
    );
}

/// One script builds the bundle for `just web` and for the release job; CI passes
/// `--require` so a missing wasm-pack fails the release instead of shipping an
/// archive without a UI.
#[test]
fn the_release_job_builds_the_bundle_and_refuses_to_skip_it() {
    let setup = read(".github/build-setup.yml");
    assert!(
        setup.contains("tools/webui-bundle.sh --require"),
        "the dist build-setup no longer builds the bundle"
    );
    assert!(
        setup.contains("tool: wasm-pack"),
        "the dist build-setup no longer installs wasm-pack"
    );
}

/// `release.yml` is generated from `dist-workspace.toml` + `build-setup.yml`
/// (`just dist-generate`); a change to the setup that was never regenerated would
/// leave CI running the old steps.
#[test]
fn the_generated_workflow_is_in_step_with_build_setup() {
    let release = read(".github/workflows/release.yml");
    assert!(
        release.contains("tools/webui-bundle.sh --require"),
        "run `just dist-generate` and commit .github/workflows/release.yml"
    );
}

/// The script gates the bundle at the same number `tests/web_wasm.rs` asserts —
/// two copies of the T60.7 measurement, so they are held together here.
#[test]
fn the_script_gate_matches_the_measured_size_gate() {
    let script = read("tools/webui-bundle.sh");
    let gate = script
        .lines()
        .find_map(|l| l.trim().strip_prefix("gate="))
        .expect("gate= in tools/webui-bundle.sh");
    let test = read("tests/web_wasm.rs");
    let measured = test
        .lines()
        .find_map(|l| l.split("WASM_SIZE_GATE: u64 = ").nth(1))
        .expect("WASM_SIZE_GATE in tests/web_wasm.rs")
        .trim_end_matches(';')
        .replace('_', "");
    assert_eq!(gate, measured, "T60.7 gate drifted between script and test");
}

/// A bundle built here must be the one `build.rs` embeds: the script writes into
/// the directory it reads. Skipped when nothing has been built yet.
#[test]
fn a_built_bundle_lands_where_build_rs_embeds_it() {
    let pkg = repo("crates/rtok-webui/pkg");
    if !Path::new(&pkg).is_dir() {
        eprintln!("skip: no bundle built — run `just web-bundle`");
        return;
    }
    for name in ["rtok_webui.js", "rtok_webui_bg.wasm"] {
        assert!(
            pkg.join(name).is_file(),
            "{name} missing from {}",
            pkg.display()
        );
    }
}
