// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! The `rtok doctor --fix` checklist (T331.7): what a terminal user sees before anything is
//! written. Every removable finding is a numbered line; a number toggles it, `k N` makes copy N
//! the kept one of its duplicate, `d` shows the diff of each file, `y` asks for the last
//! confirmation, `q` leaves. Items in a shared project file start unselected, since the change
//! reaches teammates. All input goes through [`Prompt`], so tests script the answers.

use std::collections::BTreeSet;
use std::io::{self, Write};
use std::path::Path;

use super::fix::removable;
use super::hooks::Problem;

/// One question to the user: print `screen`, then read a line. `None` at the end of the input.
pub trait Prompt {
    fn ask(&mut self, screen: &str) -> Option<String>;
}

/// The terminal: the screen on stdout, the answer from stdin.
pub struct Terminal;

impl Prompt for Terminal {
    fn ask(&mut self, screen: &str) -> Option<String> {
        print!("{screen}");
        io::stdout().flush().ok()?;
        let mut line = String::new();
        (io::stdin().read_line(&mut line).ok()? > 0).then_some(line)
    }
}

/// How the checklist ended.
#[derive(Debug, PartialEq)]
pub enum Choice {
    Cancel,
    /// Write everything removable except the entries (`source`, `path`) listed here.
    Apply(BTreeSet<(String, String)>),
}

/// An entry's identity: its file and its key path.
pub type Key = (String, String);

pub fn key(x: &Problem) -> Key {
    (x.source.clone(), x.path.clone())
}

/// The finding of `found[i]`'s entry that says it is a copy of a duplicate, if it has one.
pub fn copy_of(found: &[Problem], i: usize) -> usize {
    let k = key(&found[i]);
    (0..found.len())
        .find(|&j| key(&found[j]) == k && found[j].kind.starts_with("duplicate-"))
        .unwrap_or(i)
}

/// Make copy `i` the kept one of its group; the old kept copy becomes an extra. Refused for what
/// the host decides (a server a wider scope overrides) and for a kept copy the user may not edit.
pub(super) fn keep_copy(found: &mut [Problem], i: usize) -> Result<usize, &'static str> {
    let (kind, group) = (found[i].kind, found[i].group);
    if !found[i].kind.starts_with("duplicate-") {
        return Err("only a copy of a duplicate has a kept copy");
    }
    if !found[i].detail.starts_with("runs ") {
        return Err("the host decides which of these is used; it cannot be changed here");
    }
    let old = found
        .iter()
        .position(|x| x.kind == kind && x.group == group && x.keep)
        .ok_or("that copy is not part of a duplicate")?;
    if !found[old].fixable {
        return Err("the kept copy is not yours to edit, so it stays");
    }
    let kept = found[old].detail.split(" (").next().unwrap_or_default();
    let kept = format!("{kept} (your choice)");
    found[old].detail = std::mem::replace(&mut found[i].detail, kept);
    found[old].keep = false;
    found[i].keep = true;
    Ok(old)
}

fn screen(
    found: &[Problem],
    items: &[usize],
    off: &BTreeSet<Key>,
    shared: &dyn Fn(&Problem) -> bool,
    note: &str,
) -> String {
    let mut out =
        format!("{note}Fix selected: toggle N, k N keeps copy N, d diff, y write, q quit\n");
    for (n, &i) in items.iter().enumerate() {
        let x = &found[i];
        let mark = if off.contains(&key(x)) { ' ' } else { 'x' };
        let what = match x.kind {
            "broken-hook" => format!("broken hook {} `{}`: {}", x.event, x.command, x.detail),
            "duplicate-mcp" => format!("extra mcp {} `{}`", x.path, x.command),
            _ => format!("extra hook {} `{}`", x.event, x.command),
        };
        let kept = found
            .iter()
            .find(|k| k.kind == x.kind && k.group.is_some() && k.group == x.group && k.keep)
            .map(|k| format!("; kept in {}", k.source))
            .unwrap_or_default();
        let team = if shared(x) {
            " (shared project file)"
        } else {
            ""
        };
        out.push_str(&format!(
            " {:>2} [{mark}] {what}\n      {}{team}{kept}\n",
            n + 1,
            x.source
        ));
    }
    out + "> "
}

/// The entries a project's shared file holds start unselected: the change reaches teammates, while
/// its `.local` files and the user's own files do not.
pub fn shared_in<'a>(
    cwd: Option<&'a Path>,
    home: Option<&'a Path>,
) -> impl Fn(&Problem) -> bool + 'a {
    move |x| {
        let path = Path::new(&x.source);
        cwd.is_some_and(|c| {
            path.starts_with(c)
                && home != Some(c)
                && !path
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().contains(".local"))
        })
    }
}

/// The entries that start deselected.
pub fn defaults(
    found: &[Problem],
    kinds: &[&str],
    shared: &dyn Fn(&Problem) -> bool,
) -> BTreeSet<Key> {
    let removing = found.iter().filter(|x| removable(x, kinds));
    removing.filter(|x| shared(x)).map(key).collect()
}

/// One line per entry: a hook that is both broken and an extra copy is listed once.
pub fn items(found: &[Problem], kinds: &[&str]) -> Vec<usize> {
    let mut seen = BTreeSet::new();
    (0..found.len())
        .filter(|&i| removable(&found[i], kinds) && seen.insert(key(&found[i])))
        .collect()
}

/// Run the checklist over `found` (changed in place by `k N`). `preview` renders the diffs of
/// the current selection; nothing here writes.
pub fn run(
    found: &mut [Problem],
    kinds: &[&str],
    shared: &dyn Fn(&Problem) -> bool,
    preview: &dyn Fn(&[Problem], &BTreeSet<Key>) -> String,
    prompt: &mut dyn Prompt,
) -> Choice {
    let mut off = defaults(found, kinds, shared);
    let mut note = String::new();
    loop {
        let items = items(found, kinds);
        let on = items
            .iter()
            .filter(|&&i| !off.contains(&key(&found[i])))
            .count();
        let Some(line) = prompt.ask(&screen(found, &items, &off, shared, &note)) else {
            return Choice::Cancel;
        };
        note.clear();
        let words: Vec<&str> = line.split_whitespace().collect();
        let pick = |w: &str| {
            w.parse::<usize>()
                .ok()
                .and_then(|n| items.get(n.checked_sub(1)?).copied())
        };
        match words.as_slice() {
            ["q"] => return Choice::Cancel,
            ["d"] => note = format!("{}\n", preview(found, &off)),
            ["y"] if on == 0 => note = "nothing is selected\n".into(),
            ["y"] => {
                let ask = format!("{}\nWrite {on} change(s)? [y/N] ", preview(found, &off));
                if prompt
                    .ask(&ask)
                    .is_some_and(|a| a.trim().eq_ignore_ascii_case("y"))
                {
                    return Choice::Apply(off);
                }
                note = "not written\n".into();
            }
            ["k", n] => match pick(n) {
                Some(i) => match keep_copy(found, copy_of(found, i)) {
                    // The old kept copy is a new line; it starts as any other would.
                    Ok(old) => {
                        off.remove(&key(&found[i]));
                        if shared(&found[old]) {
                            off.insert(key(&found[old]));
                        }
                    }
                    Err(why) => note = format!("{why}\n"),
                },
                None => note = "no such item\n".into(),
            },
            [n] => match pick(n) {
                Some(i) => {
                    let k = key(&found[i]);
                    if !off.remove(&k) {
                        off.insert(k);
                    }
                }
                None => note = "answer a number, k N, d, y or q\n".into(),
            },
            _ => note = "answer a number, k N, d, y or q\n".into(),
        }
    }
}
