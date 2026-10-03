// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

// A mocked machine for the doctor fix flow (T331.12): the same request/answer shape as the
// server, over three fixed findings, so the page and its tests run without a host. It mirrors
// the server's rules (shared files start unselected, a keep swap moves the removal to the other
// copy) but removes nothing real; `apply` only reports what it would have written.
import type { Fixed, Item, Plan, Ref, Selection } from "./snapshot.gen";

const USER = "/home/me/.claude/settings.json";
const PROJECT = "/work/app/.claude/settings.json";
const BROKEN_PATH = "hooks.Stop[0].hooks[0]";
const DUP_PATH = "hooks.PreToolUse[0].hooks[0]";

const base = {
  agent: "claude",
};

const has = (refs: Ref[], source: string, path: string) =>
  refs.some((r) => r.source === source && r.path === path);

const brokenIn = (source: string, shared: boolean): Item => ({
  ...base,
  source,
  path: BROKEN_PATH,
  kind: "broken-hook",
  label: "/opt/gone/stop.sh",
  detail: "the command does not exist",
  kept_in: null,
  can_keep: false,
  shared,
  selected: !shared,
});

export interface MockMachine {
  plan(selection: Selection): Plan;
  apply(selection: Selection): Fixed;
}

export function mockMachine(): MockMachine {
  const items = (sel: Selection): Item[] => {
    // The last swap names the copy that stays; the other one is the removable entry.
    const kept = sel.keep.at(-1)?.source === USER ? USER : PROJECT;
    const gone = kept === USER ? PROJECT : USER;
    const shared = gone === PROJECT;
    const dup: Item = {
      ...base,
      source: gone,
      path: DUP_PATH,
      kind: "duplicate-hook",
      label: "rtok hook pre-tool",
      detail: "also registered in another file",
      kept_in: kept,
      can_keep: true,
      shared,
      selected: !shared,
    };
    return [brokenIn(USER, false), brokenIn(PROJECT, true), dup].map((i) => ({
      ...i,
      // After a swap the defaults are recomputed, so a toggle flips the new default.
      selected: has(sel.toggled, i.source, i.path) ? !i.selected : i.selected,
    }));
  };
  const chosen = (sel: Selection) => items(sel).filter((i) => i.selected);
  return {
    plan(sel) {
      const picked = chosen(sel);
      const files = [...new Set(picked.map((i) => i.source))];
      const diff = files
        .map(
          (f) =>
            `--- ${f}\n+++ ${f}\n${picked
              .filter((i) => i.source === f)
              .map((i) => `-  ${i.path}: ${i.label}`)
              .join("\n")}`,
        )
        .join("\n");
      return { items: items(sel), diff, refused: [] };
    },
    apply(sel) {
      const picked = chosen(sel);
      return {
        code: 0,
        text: `${picked.length} ${picked.length === 1 ? "entry" : "entries"} removed from ${new Set(picked.map((i) => i.source)).size} file(s); backups kept in _backup/\n`,
      };
    },
  };
}
