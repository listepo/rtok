import type { Snapshot } from "./api/snapshot.gen";

// Mirrors `model::pages()` (src/web/model.rs): page id and the snapshot field it reads. The
// route tree, sidebar and tab bar are all built from this one list, so a page cannot be
// routable and missing from the nav. Rust's `tests/surface_parity.rs` keeps the ids honest.
export const PAGES = [
  { id: "overview", field: "usage" },
  { id: "plugins", field: "plugins" },
  { id: "calls", field: "calls" },
  { id: "sessions", field: "sessions" },
  { id: "doctor", field: "doctor" },
  { id: "logs", field: "logs" },
  { id: "skills", field: "skills" },
  { id: "stats", field: "stats" },
  { id: "graph", field: "graph" },
  { id: "hosts", field: "hosts" },
  { id: "config", field: "config" },
  { id: "services", field: "services" },
  { id: "worktrees", field: "worktrees" },
] as const satisfies readonly { id: string; field: keyof Snapshot }[];

export type Page = (typeof PAGES)[number];
