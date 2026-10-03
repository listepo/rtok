// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! Go: `replace` directives of `go.mod`/`go.work` and the `use` directories of `go.work`.

use std::path::Path;

use super::Ctx;
use crate::fs::normalize;

/// The `(verb, argument line)` of every directive of a `go.mod`/`go.work` file, block forms
/// (`replace ( … )`) flattened.
fn go_directives(text: &str, verbs: &[&str]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut block: Option<String> = None;
    for line in text.lines() {
        let line = line.split("//").next().unwrap_or_default().trim();
        if line == ")" {
            block = None;
        } else if let Some(verb) = &block {
            out.push((verb.clone(), line.to_string()));
        } else if let Some(verb) = verbs
            .iter()
            .find(|v| line.strip_prefix(**v).is_some_and(|r| r.trim() == "("))
        {
            block = Some(verb.to_string());
        } else if let Some((verb, rest)) = verbs
            .iter()
            .find_map(|v| Some((*v, line.strip_prefix(*v)?.strip_prefix([' ', '\t'])?)))
        {
            out.push((verb.to_string(), rest.trim().to_string()));
        }
    }
    out
}

/// A Go path argument without its quotes.
fn go_path(s: &str) -> &str {
    s.trim_matches(['"', '`'])
}

pub(super) fn go(c: &mut Ctx) {
    let work = c.root.join("go.work");
    let mut dirs = vec![c.root.clone()];
    if let Some(text) = c.read(&work) {
        for (verb, rest) in go_directives(&text, &["use", "replace"]) {
            if verb == "use" {
                let used = go_path(&rest).to_string();
                c.add(
                    &work,
                    &c.root.clone(),
                    &used,
                    format!("go.work use {used}"),
                    false,
                );
                dirs.push(normalize(&c.root, Path::new(&used)));
            } else {
                go_replace(c, &work, &c.root.clone(), &rest);
            }
        }
    }
    for dir in dirs {
        let file = dir.join("go.mod");
        if let Some(text) = c.read(&file) {
            for (_, rest) in go_directives(&text, &["replace"]) {
                go_replace(c, &file, &dir, &rest);
            }
        }
    }
}

/// `old [v] => new [v]`: only a `new` that is a path, never a module path and version.
fn go_replace(c: &mut Ctx, file: &Path, dir: &Path, spec: &str) {
    let Some((old, new)) = spec.split_once("=>") else {
        return;
    };
    let path = go_path(new.split_whitespace().next().unwrap_or_default());
    if path.starts_with("./") || path.starts_with("../") || Path::new(path).is_absolute() {
        let old = old.split_whitespace().next().unwrap_or_default();
        c.add(file, dir, path, format!("go replace {old}"), false);
    }
}
