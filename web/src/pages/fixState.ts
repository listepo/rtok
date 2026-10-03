// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

// State of the "Fix selected" panel (T331.12). The server owns the checklist and its defaults,
// so the page keeps only what the user changed since them: a request is always that delta.
import type { Fixed, Plan, Ref, Selection } from "../api/snapshot.gen";

export type FixPhase = "loading" | "ready" | "confirming" | "applying" | "done" | "error";

export interface FixState {
  selection: Selection;
  plan: Plan | null;
  phase: FixPhase;
  result: Fixed | null;
  error: string | null;
  // Bumped on every selection change so a late plan for an old selection is dropped.
  seq: number;
}

export type FixAction =
  | { type: "toggle"; ref: Ref }
  | { type: "keep"; ref: Ref }
  | { type: "planned"; seq: number; plan: Plan }
  | { type: "ask" }
  | { type: "cancel" }
  | { type: "applying" }
  | { type: "applied"; fixed: Fixed }
  | { type: "failed"; error: string }
  | { type: "again" };

export const initialFix: FixState = {
  selection: { keep: [], toggled: [] },
  plan: null,
  phase: "loading",
  result: null,
  error: null,
  seq: 0,
};

export const refKey = (r: Ref) => `${r.source}\u0000${r.path}`;

const same = (a: Ref) => (b: Ref) => refKey(a) === refKey(b);

export function selectedCount(plan: Plan | null): number {
  return plan ? plan.items.filter((i) => i.selected).length : 0;
}

export function fixReducer(state: FixState, action: FixAction): FixState {
  switch (action.type) {
    case "toggle": {
      const { toggled, keep } = state.selection;
      const next = toggled.some(same(action.ref))
        ? toggled.filter((r) => !same(action.ref)(r))
        : [...toggled, action.ref];
      return reselect(state, { keep, toggled: next });
    }
    case "keep":
      // The server recomputes the defaults after a swap, so the old toggles no longer
      // say what they did; the swap starts the selection over from the new defaults.
      return reselect(state, { keep: [...state.selection.keep, action.ref], toggled: [] });
    case "planned":
      if (action.seq !== state.seq) return state;
      return { ...state, plan: action.plan, phase: "ready", error: null };
    case "ask":
      return selectedCount(state.plan) > 0 && state.phase === "ready"
        ? { ...state, phase: "confirming" }
        : state;
    case "cancel":
      return state.phase === "confirming" ? { ...state, phase: "ready" } : state;
    case "applying":
      return state.phase === "confirming" ? { ...state, phase: "applying" } : state;
    case "applied":
      return { ...state, phase: "done", result: action.fixed, error: null };
    case "failed":
      return { ...state, phase: "error", error: action.error };
    case "again":
      return { ...initialFix, seq: state.seq + 1 };
  }
}

// A change while the user is confirming withdraws the confirmation: what they confirmed is no
// longer what the diff shows.
function reselect(state: FixState, selection: Selection): FixState {
  if (state.phase === "applying" || state.phase === "done") return state;
  return { ...state, selection, phase: "loading", seq: state.seq + 1 };
}
