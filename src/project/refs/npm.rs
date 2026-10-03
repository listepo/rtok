// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! npm, pnpm and yarn: `file:`/`link:`/`workspace:` dependencies and workspace members.

use serde_json::Value;

use super::Ctx;

const NPM_LOCAL: [&str; 3] = ["file:", "link:", "workspace:"];

pub(super) fn npm(c: &mut Ctx) {
    let manifest = c.root.join("package.json");
    let root_pkg = c.read(&manifest).and_then(|t| {
        serde_json::from_str::<Value>(&t)
            .map_err(|e| c.warn(&manifest, &e))
            .ok()
    });
    let mut patterns: Vec<String> = Vec::new();
    if let Some(ws) = root_pkg.as_ref().and_then(|p| p.get("workspaces")) {
        // Yarn and npm: an array, or an object whose `packages` is the array.
        let list = ws.get("packages").unwrap_or(ws);
        patterns.extend(
            list.as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(String::from),
        );
    }
    let pnpm = c.root.join("pnpm-workspace.yaml");
    if let Some(text) = c.read(&pnpm) {
        #[derive(serde::Deserialize, Default)]
        struct Workspace {
            #[serde(default)]
            packages: Vec<String>,
        }
        match serde_saphyr::from_str::<Workspace>(&text) {
            Ok(w) => patterns.extend(w.packages),
            Err(e) => c.warn(&pnpm, &e),
        }
    }
    let mut dirs = vec![c.root.clone()];
    dirs.extend(c.members(&manifest, &patterns, "package.json", "npm workspace"));
    for dir in dirs {
        let file = dir.join("package.json");
        let pkg = if dir == c.root {
            root_pkg.clone()
        } else {
            c.read(&file)
                .and_then(|t| serde_json::from_str(&t).map_err(|e| c.warn(&file, &e)).ok())
        };
        let Some(pkg) = pkg else { continue };
        for table in [
            "dependencies",
            "devDependencies",
            "optionalDependencies",
            "peerDependencies",
        ] {
            for (name, spec) in pkg
                .get(table)
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
            {
                let local = spec
                    .as_str()
                    .and_then(|s| NPM_LOCAL.iter().find_map(|p| s.strip_prefix(p)));
                // `workspace:*` and `workspace:^1` name a workspace package, not a path.
                if let Some(path) = local.filter(|p| p.starts_with(['.', '/'])) {
                    c.add(&file, &dir, path, format!("npm {table} {name}"), false);
                }
            }
        }
    }
}
