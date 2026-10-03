// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! Cargo manifests: `path` dependencies, `[patch]` and workspace members.

use toml_edit::{Item, TableLike};

use super::{Ctx, dep_paths, strings};

pub(super) fn cargo(c: &mut Ctx) {
    let manifest = c.root.join("Cargo.toml");
    let Some(doc) = c.toml(&manifest) else { return };
    let members = strings(doc.get("workspace").and_then(|w| w.get("members")));
    let mut dirs = vec![c.root.clone()];
    dirs.extend(c.members(&manifest, &members, "Cargo.toml", "cargo workspace"));
    for dir in dirs {
        let file = dir.join("Cargo.toml");
        let doc = if dir == c.root {
            Some(doc.clone())
        } else {
            c.toml(&file)
        };
        let Some(doc) = doc else { continue };
        let kinds = ["dependencies", "dev-dependencies", "build-dependencies"];
        let mut tables: Vec<(&str, Option<&dyn TableLike>)> = kinds
            .iter()
            .map(|k| ("path dependency", doc.get(k).and_then(Item::as_table_like)))
            .collect();
        tables.push((
            "workspace dependency",
            doc.get("workspace")
                .and_then(|w| w.get("dependencies"))
                .and_then(Item::as_table_like),
        ));
        for (_, t) in doc
            .get("target")
            .and_then(Item::as_table_like)
            .into_iter()
            .flat_map(|t| t.iter())
        {
            tables.extend(
                kinds
                    .iter()
                    .map(|k| ("path dependency", t.get(k).and_then(Item::as_table_like))),
            );
        }
        for (_, t) in doc
            .get("patch")
            .and_then(Item::as_table_like)
            .into_iter()
            .flat_map(|t| t.iter())
        {
            tables.push(("[patch]", t.as_table_like()));
        }
        for (what, t) in tables {
            for (name, path) in dep_paths(t) {
                c.add(&file, &dir, &path, format!("cargo {what} {name}"), false);
            }
        }
    }
}
