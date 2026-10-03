// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! Python: PEP 508 file references, Poetry and uv path sources, uv workspace members and
//! `requirements.txt` paths.

use toml_edit::Item;

use super::{Ctx, dep_paths, strings};

/// A PEP 508 `name @ file:…` direct reference, as the path.
fn pep508_file(req: &str) -> Option<&str> {
    let url = req.split_once(" @ ")?.1.split(';').next()?.trim();
    let path = url.strip_prefix("file:")?;
    Some(path.strip_prefix("//").unwrap_or(path))
}

pub(super) fn python(c: &mut Ctx) {
    let file = c.root.join("pyproject.toml");
    let Some(doc) = c.toml(&file) else {
        requirements(c);
        return;
    };
    let root = c.root.clone();
    let mut found: Vec<(String, String)> = Vec::new();
    let project = doc.get("project");
    let groups = [
        project.and_then(|p| p.get("optional-dependencies")),
        doc.get("dependency-groups"),
    ];
    let lists = std::iter::once(strings(project.and_then(|p| p.get("dependencies")))).chain(
        groups
            .iter()
            .flatten()
            .filter_map(|g| g.as_table_like())
            .flat_map(|t| t.iter().map(|(_, v)| strings(Some(v)))),
    );
    for req in lists.flatten() {
        if let Some(p) = pep508_file(&req) {
            let name = req.split(['@', ' ']).next().unwrap_or_default();
            found.push((p.to_string(), format!("python path dependency {name}")));
        }
    }
    let tool = doc.get("tool");
    let poetry = tool.and_then(|t| t.get("poetry"));
    let mut poetry_tables = vec![poetry.and_then(|p| p.get("dependencies"))];
    poetry_tables.extend(
        poetry
            .and_then(|p| p.get("group"))
            .and_then(Item::as_table_like)
            .into_iter()
            .flat_map(|g| g.iter().map(|(_, v)| v.get("dependencies"))),
    );
    let uv_sources = tool
        .and_then(|t| t.get("uv"))
        .and_then(|u| u.get("sources"));
    for t in poetry_tables.into_iter().chain([uv_sources]) {
        found.extend(
            dep_paths(t.and_then(Item::as_table_like))
                .into_iter()
                .map(|(n, p)| (p, format!("python path dependency {n}"))),
        );
    }
    for (path, reason) in found {
        c.add(&file, &root, &path, reason, false);
    }
    let members = strings(
        tool.and_then(|t| t.get("uv"))
            .and_then(|u| u.get("workspace"))
            .and_then(|w| w.get("members")),
    );
    c.members(&file, &members, "pyproject.toml", "uv workspace");
    requirements(c);
}

/// `-e ../pkg` and bare `./pkg` lines of `requirements.txt`.
fn requirements(c: &mut Ctx) {
    let file = c.root.join("requirements.txt");
    let Some(text) = c.read(&file) else { return };
    let root = c.root.clone();
    for line in text.lines().map(str::trim) {
        let path = line
            .strip_prefix("-e ")
            .or_else(|| line.strip_prefix("--editable "))
            .unwrap_or(line)
            .trim();
        if path.starts_with("./") || path.starts_with("../") {
            c.add(
                &file,
                &root,
                path,
                format!("python requirement {path}"),
                false,
            );
        }
    }
}
