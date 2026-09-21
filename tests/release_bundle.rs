//! T81: the web UI ships with the release archive, and the three places that say so
//! stay in step. Pure file reads — the archive itself is built in CI, but a dropped
//! `include`, a hand-edited `release.yml` or a drifted size gate is caught here.

use std::path::{Path, PathBuf};

fn repo(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)
}

fn read(rel: &str) -> String {
    let path = repo(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// The bundle rides in every archive, next to the binary — the first directory
/// `rtok web` looks at (`pkg_dir` in `src/web/mod.rs`).
#[test]
fn dist_archives_carry_the_wasm_bundle() {
    let manifest = read("Cargo.toml");
    let include = manifest
        .lines()
        .find(|l| l.starts_with("include = ["))
        .expect("[package.metadata.dist] include");
    assert!(
        include.contains("crates/rtok-webui/pkg/"),
        "dist include lost the web UI bundle: {include}"
    );
}

/// `rtok web` resolves `pkg/` beside the executable, which is where dist unpacks a
/// top-level `include`d directory. If that candidate is ever dropped, the archive
/// becomes dead weight and an installed `rtok web` silently loses its UI again.
#[test]
fn the_server_still_looks_beside_the_executable() {
    let web = read("src/web/mod.rs");
    assert!(
        web.contains(r#"bin.join("pkg")"#),
        "src/web/mod.rs no longer looks for pkg/ next to current_exe()"
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

/// A bundle built here must be the one the archive would carry: the script writes
/// into the directory `include` names. Skipped when nothing has been built yet.
#[test]
fn a_built_bundle_lands_where_dist_picks_it_up() {
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
