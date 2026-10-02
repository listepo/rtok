// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! Reference discovery (T329.7): the directories outside a project's root that its manifests
//! point at, as `(directory, reason)`, plus a warning for each path that does not exist. A pure
//! read of the manifests, in the order Cargo, npm/pnpm/yarn, Go, Python, `.gitmodules`: nothing
//! is registered or indexed here (T329.8 follows the references). Only local source is returned;
//! a registry dependency names no path, so it never appears. The root manifest and the manifests
//! of its workspace members are read (members often hold the `path` dependencies); a member that
//! lives outside the root is itself a reference. A path inside the root is the project's own code
//! and is skipped, except for a submodule, which is a separate repository. A workspace glob is
//! matched one directory level per segment, `**` counting as `*`.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use toml_edit::{DocumentMut, Item, TableLike};

mod cargo;
mod go;
mod npm;
mod python;

use crate::fs::normalize;

/// One referenced directory and the manifest entry that names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub dir: PathBuf,
    pub reason: String,
}

/// Everything [`discover`] found: the existing directories, and what could not be followed.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Found {
    pub refs: Vec<Reference>,
    pub warnings: Vec<String>,
}

/// The directories outside `root` that the manifests under it reference, one entry per
/// directory (the first source to name it wins).
pub fn discover(root: &Path) -> Found {
    let mut c = Ctx {
        root: normalize(Path::new("/"), root),
        found: Found::default(),
    };
    cargo::cargo(&mut c);
    npm::npm(&mut c);
    go::go(&mut c);
    python::python(&mut c);
    submodules(&mut c);
    c.found
}

struct Ctx {
    root: PathBuf,
    found: Found,
}

impl Ctx {
    /// Record `written` (as the manifest `from` spells it), resolved against `base`.
    fn add(&mut self, from: &Path, base: &Path, written: &str, reason: String, inside_ok: bool) {
        if written.is_empty() || written.starts_with('~') {
            return;
        }
        let dir = normalize(base, Path::new(written));
        // A path that contains the root would swallow the project instead of referencing it.
        let own = dir.starts_with(&self.root) || self.root.starts_with(&dir);
        if own && !inside_ok {
            return;
        }
        if dir.is_dir() {
            if !self.found.refs.iter().any(|r| r.dir == dir) {
                self.found.refs.push(Reference { dir, reason });
            }
        // A file is a tarball or a wheel, not a directory to index.
        } else if !dir.exists() {
            let from = from.strip_prefix(&self.root).unwrap_or(from);
            let w = format!("{}: references {written}, not found", from.display());
            if !self.found.warnings.contains(&w) {
                self.found.warnings.push(w);
            }
        }
    }

    /// The text of a manifest. A missing file is no finding; one that cannot be read is.
    fn read(&mut self, path: &Path) -> Option<String> {
        match std::fs::read_to_string(path) {
            Ok(text) => Some(text),
            Err(e) if e.kind() == ErrorKind::NotFound => None,
            Err(e) => {
                self.warn(path, &e);
                None
            }
        }
    }

    fn warn(&mut self, path: &Path, e: &dyn std::fmt::Display) {
        let p = path.strip_prefix(&self.root).unwrap_or(path);
        self.found
            .warnings
            .push(format!("{}: not read: {e}", p.display()));
    }

    fn toml(&mut self, path: &Path) -> Option<DocumentMut> {
        let text = self.read(path)?;
        text.parse().map_err(|e| self.warn(path, &e)).ok()
    }

    /// Workspace member patterns of a manifest into the directories to read, naming the ones
    /// outside the root as references. `marker` is the manifest a glob match must hold.
    fn members(
        &mut self,
        from: &Path,
        patterns: &[String],
        marker: &str,
        what: &str,
    ) -> Vec<PathBuf> {
        let base = from.parent().unwrap_or(&self.root).to_path_buf();
        let mut dirs = Vec::new();
        for pat in patterns.iter().filter(|p| !p.starts_with('!')) {
            let glob = pat.contains(['*', '?', '[', '{']);
            for dir in expand(&base, pat, marker) {
                let written = if glob {
                    dir.to_string_lossy().into_owned()
                } else {
                    pat.clone()
                };
                self.add(from, &base, &written, format!("{what} member {pat}"), false);
                dirs.push(dir);
            }
        }
        dirs
    }
}

/// `pattern` below `base`: the directory itself when it has no glob, else the existing matches
/// that hold `marker`.
fn expand(base: &Path, pattern: &str, marker: &str) -> Vec<PathBuf> {
    if !pattern.contains(['*', '?', '[', '{']) {
        return vec![normalize(base, Path::new(pattern))];
    }
    let mut dirs = vec![base.to_path_buf()];
    for seg in Path::new(pattern)
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
    {
        let seg = if seg == "**" { "*".into() } else { seg };
        if !seg.contains(['*', '?', '[', '{']) {
            dirs = dirs
                .into_iter()
                .map(|d| normalize(&d, Path::new(seg.as_ref())))
                .collect();
            continue;
        }
        let Ok(glob) = globset::Glob::new(&seg).map(|g| g.compile_matcher()) else {
            return Vec::new();
        };
        dirs = dirs
            .iter()
            .filter_map(|d| std::fs::read_dir(d).ok())
            .flatten()
            .flatten()
            .filter(|e| glob.is_match(e.file_name()))
            .map(|e| e.path())
            .collect();
        dirs.sort();
    }
    dirs.retain(|d| d.join(marker).is_file());
    dirs
}

fn strings(v: Option<&Item>) -> Vec<String> {
    v.and_then(Item::as_array)
        .into_iter()
        .flatten()
        .filter_map(|s| s.as_str().map(String::from))
        .collect()
}

/// `path = "…"` of each dependency of a table, with the dependency's name.
fn dep_paths(t: Option<&dyn TableLike>) -> Vec<(String, String)> {
    t.into_iter()
        .flat_map(|t| t.iter())
        .filter_map(|(name, v)| {
            let p = v.as_table_like()?.get("path")?.as_str()?;
            Some((name.to_string(), p.to_string()))
        })
        .collect()
}

/// `path = …` of each `[submodule "…"]` section of `.gitmodules`.
fn submodules(c: &mut Ctx) {
    let file = c.root.join(".gitmodules");
    let Some(text) = c.read(&file) else { return };
    let root = c.root.clone();
    let mut section = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            section = line.starts_with("[submodule");
        } else if let (true, Some((key, value))) = (section, line.split_once('='))
            && key.trim() == "path"
        {
            let path = value.trim().trim_matches('"');
            c.add(&file, &root, path, format!("git submodule {path}"), true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::tmp_dir;

    /// A workspace of sibling directories: `proj` is the root under test, the rest are outside it.
    struct Fx {
        base: PathBuf,
    }

    impl Fx {
        fn new() -> Self {
            Fx {
                base: tmp_dir("refs"),
            }
        }

        fn file(&self, path: &str, text: &str) -> &Self {
            let p = self.base.join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
            self
        }

        fn dir(&self, path: &str) -> &Self {
            std::fs::create_dir_all(self.base.join(path)).unwrap();
            self
        }

        /// `(directory, reason)` below the fixture base, and the warnings.
        fn run(&self) -> (Vec<(String, String)>, Vec<String>) {
            let found = discover(&self.base.join("proj"));
            let refs = found
                .refs
                .iter()
                .map(|r| {
                    let dir = r.dir.strip_prefix(&self.base).unwrap();
                    // `/`-joined, so the expectations hold on Windows too.
                    (
                        dir.display().to_string().replace('\\', "/"),
                        r.reason.clone(),
                    )
                })
                .collect();
            (refs, found.warnings)
        }
    }

    fn pairs(v: &[(&str, &str)]) -> Vec<(String, String)> {
        v.iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect()
    }

    #[test]
    fn cargo_path_patch_and_workspace_references() {
        let fx = Fx::new();
        fx.file(
            "proj/Cargo.toml",
            r#"[workspace]
members = ["crates/*", "../outer", "../ghost-member"]
[workspace.dependencies]
wsdep = { path = "../wsdep" }
[dependencies]
serde = "1"
shared = { path = "../shared" }
inner = { path = "crates/a" }
ghost = { path = "../ghost" }
[dev-dependencies.dev]
path = "../dev"
[target.'cfg(unix)'.dependencies]
unix = { path = "../unix" }
[patch.crates-io]
patched = { path = "../patched" }
registry-patch = { git = "https://example.com/x" }
"#,
        );
        fx.file(
            "proj/crates/a/Cargo.toml",
            "[dependencies]\nfar = { path = \"../../../far\" }\nown = { path = \"../b\" }\n",
        );
        fx.dir("proj/crates/b").dir("proj/crates/README");
        for d in ["outer", "wsdep", "shared", "dev", "unix", "patched", "far"] {
            fx.dir(d);
        }
        let (refs, warnings) = fx.run();
        assert_eq!(
            refs,
            pairs(&[
                ("outer", "cargo workspace member ../outer"),
                ("shared", "cargo path dependency shared"),
                ("dev", "cargo path dependency dev"),
                ("wsdep", "cargo workspace dependency wsdep"),
                ("unix", "cargo path dependency unix"),
                ("patched", "cargo [patch] patched"),
                ("far", "cargo path dependency far"),
            ])
        );
        assert_eq!(
            warnings,
            [
                "Cargo.toml: references ../ghost-member, not found",
                "Cargo.toml: references ../ghost, not found"
            ]
        );
    }

    #[test]
    fn npm_pnpm_and_yarn_local_references() {
        let fx = Fx::new();
        fx.file(
            "proj/package.json",
            r#"{"workspaces": {"packages": ["packages/*", "../ws", "!packages/skip"]},
                "dependencies": {"react": "^18", "lib": "file:../lib", "wsp": "workspace:*",
                                 "tar": "file:../pack.tgz", "gone": "link:../gone"},
                "devDependencies": {"linked": "link:../linked", "rel": "workspace:../wrel"}}"#,
        );
        fx.file(
            "proj/pnpm-workspace.yaml",
            "packages:\n  - '../pnpm-pkg'\n  - 'apps/*'\n",
        );
        fx.file(
            "proj/packages/p/package.json",
            r#"{"dependencies": {"deep": "file:../../../deep"}}"#,
        );
        fx.file("pack.tgz", "");
        for d in ["lib", "linked", "wrel", "ws", "pnpm-pkg", "deep"] {
            fx.dir(d);
        }
        let (refs, warnings) = fx.run();
        let dirs: Vec<&str> = refs.iter().map(|(d, _)| d.as_str()).collect();
        assert_eq!(dirs, ["ws", "pnpm-pkg", "lib", "linked", "wrel", "deep"]);
        assert_eq!(refs[0].1, "npm workspace member ../ws");
        assert_eq!(refs[2].1, "npm dependencies lib");
        assert_eq!(warnings, ["package.json: references ../gone, not found"]);
    }

    #[test]
    fn go_replace_and_workspace_use() {
        let fx = Fx::new();
        fx.file(
            "proj/go.mod",
            "module m\nreplace example.com/a => ../goa // local\nreplace (\n\texample.com/b v1.0.0 => example.com/c v1.2.0\n\texample.com/d => \"../god\"\n\texample.com/e => ../gone\n)\n",
        );
        fx.file(
            "proj/go.work",
            "go 1.22\nuse ./mod1\nuse (\n\t../gowork\n)\nreplace example.com/f => ../gof\n",
        );
        fx.file("proj/mod1/go.mod", "replace example.com/g => ../../gog\n");
        for d in ["goa", "god", "gowork", "gof", "gog"] {
            fx.dir(d);
        }
        let (refs, warnings) = fx.run();
        assert_eq!(
            refs,
            pairs(&[
                ("gowork", "go.work use ../gowork"),
                ("gof", "go replace example.com/f"),
                ("goa", "go replace example.com/a"),
                ("god", "go replace example.com/d"),
                ("gog", "go replace example.com/g"),
            ])
        );
        assert_eq!(warnings, ["go.mod: references ../gone, not found"]);
    }

    #[test]
    fn python_pep508_poetry_uv_and_requirements() {
        let fx = Fx::new();
        fx.file(
            "proj/pyproject.toml",
            r#"[project]
dependencies = ["requests>=2", "pep @ file:../pep", "abs @ file:///nowhere/abs", "web @ https://example.com/w.whl"]
[project.optional-dependencies]
extra = ["ex @ file:../ex ; python_version > '3.9'"]
[tool.poetry.dependencies]
python = "^3.11"
poet = { path = "../poet" }
[tool.poetry.group.dev.dependencies]
pdev = { path = "../pdev", develop = true }
[tool.uv.sources]
uvlib = { path = "../uvlib" }
[tool.uv.workspace]
members = ["../uvws/*"]
"#,
        );
        fx.file("proj/requirements.txt", "-e ../req\n./inside\nflask\n");
        fx.file("uvws/one/pyproject.toml", "");
        fx.dir("uvws/no-manifest");
        for d in ["pep", "ex", "poet", "pdev", "uvlib", "req"] {
            fx.dir(d);
        }
        let (refs, warnings) = fx.run();
        let dirs: Vec<&str> = refs.iter().map(|(d, _)| d.as_str()).collect();
        assert_eq!(
            dirs,
            ["pep", "ex", "poet", "pdev", "uvlib", "uvws/one", "req"]
        );
        assert_eq!(refs[0].1, "python path dependency pep");
        // The absolute path names no checkout on this machine.
        assert_eq!(
            warnings,
            ["pyproject.toml: references /nowhere/abs, not found"]
        );
    }

    #[test]
    fn a_submodule_inside_the_root_is_a_reference() {
        let fx = Fx::new();
        fx.file(
            "proj/.gitmodules",
            "[submodule \"lib\"]\n\tpath = vendor/lib\n\turl = https://example.com/lib.git\n[submodule \"gone\"]\n\tpath = vendor/gone\n[other]\n\tpath = not-a-submodule\n",
        );
        fx.dir("proj/vendor/lib").dir("proj/not-a-submodule");
        let (refs, warnings) = fx.run();
        assert_eq!(
            refs,
            pairs(&[("proj/vendor/lib", "git submodule vendor/lib")])
        );
        assert_eq!(warnings, [".gitmodules: references vendor/gone, not found"]);
    }

    #[test]
    fn one_entry_per_directory_in_source_order_and_nothing_without_manifests() {
        let fx = Fx::new();
        fx.dir("proj");
        assert_eq!(fx.run(), (Vec::new(), Vec::new()));
        fx.file(
            "proj/Cargo.toml",
            "[dependencies]\nx = { path = \"../x\" }\n",
        );
        fx.file(
            "proj/package.json",
            r#"{"dependencies": {"x": "file:../x"}}"#,
        );
        fx.dir("x");
        assert_eq!(fx.run().0, pairs(&[("x", "cargo path dependency x")]));
    }

    #[test]
    fn an_unparsable_manifest_is_a_warning_and_the_rest_still_read() {
        let fx = Fx::new();
        fx.file("proj/Cargo.toml", "[dependencies\n");
        fx.file("proj/package.json", "{");
        fx.file("proj/go.mod", "replace a => ../g\n");
        fx.dir("g");
        let (refs, warnings) = fx.run();
        assert_eq!(refs, pairs(&[("g", "go replace a")]));
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings[0].starts_with("Cargo.toml: not read"));
        assert!(warnings[1].starts_with("package.json: not read"));
    }

    #[test]
    fn a_path_that_contains_the_root_is_not_a_reference() {
        let fx = Fx::new();
        fx.file(
            "proj/Cargo.toml",
            "[dependencies]\nup = { path = \"..\" }\n",
        );
        assert_eq!(fx.run(), (Vec::new(), Vec::new()));
    }
}
