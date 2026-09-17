// Unit test of the pi extension (T47.3): the extension talks to `rtok` on PATH, so each case
// puts a fake `rtok` shell script (or none) first on PATH and drives the two handlers through a
// stub `pi`. Lives outside `extensions/` so pi never loads it. POSIX only: the fake is `sh`.
import assert from "node:assert/strict";
import { chmodSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import extension from "../extensions/rtok.ts";

const posix = { skip: process.platform === "win32" ? "fake rtok is a sh script" : false };

/** A dir holding `rtok` that prints `version` for `--version` and `filtered` for `filter`. */
function fakeRtok(filtered: string | null): string {
  const dir = mkdtempSync(join(tmpdir(), "rtok-pi-"));
  if (filtered !== null) {
    const body = filtered === "echo" ? "cat" : `cat >/dev/null; printf '%s' '${filtered}'`;
    writeFileSync(
      join(dir, "rtok"),
      `#!/bin/sh\ncase "$1" in\n  --version) echo "rtok 0.0.0" ;;\n  filter) ${body} ;;\nesac\n`,
    );
    chmodSync(join(dir, "rtok"), 0o755);
  }
  return dir;
}

type Handler = (event: any) => Promise<any>;

/** Load the extension against a stub `pi` with `dir` as the whole PATH (plus /bin for sh, cat). */
function load(dir: string) {
  process.env.PATH = `${dir}:/bin:/usr/bin`;
  const on: Record<string, Handler> = {};
  const entries: [string, string][] = [];
  extension({
    on: (name: string, fn: Handler) => (on[name] = fn),
    appendEntry: (kind: string, text: string) => entries.push([kind, text]),
  });
  return { on, entries };
}

test("bash calls are rewritten to one quoted `rtok run --`", posix, async () => {
  const { on } = load(fakeRtok("x"));
  const event = { toolName: "bash", input: { command: "echo it's" } };
  await on.tool_call(event);
  assert.equal(event.input.command, `rtok run -- 'echo it'"'"'s'`);
  await on.tool_call(event);
  assert.equal(event.input.command, `rtok run -- 'echo it'"'"'s'`, "never wrapped twice");
});

test("other tools are left alone", posix, async () => {
  const { on } = load(fakeRtok("x"));
  const event = { toolName: "read", input: { command: "echo hi" } };
  await on.tool_call(event);
  assert.equal(event.input.command, "echo hi");
  assert.equal(await on.tool_result({ toolName: "read", content: [{ text: "a" }] }), undefined);
});

test("missing rtok fails open and names ketch", posix, async () => {
  const { on, entries } = load(fakeRtok(null));
  const event = { toolName: "bash", input: { command: "ls" } };
  await on.tool_call(event);
  assert.equal(event.input.command, "ls", "the command runs unchanged");
  assert.equal(entries.length, 1);
  assert.match(entries[0][1], /ketch install listepo\/rtok/);
  const result = await on.tool_result({ toolName: "bash", content: [{ text: "big" }] });
  assert.equal(result, undefined, "the result passes through");
});

test("a shorter filter result replaces the bash output", posix, async () => {
  const { on } = load(fakeRtok("short [rtok expand abc]"));
  const result = await on.tool_result({
    toolName: "bash",
    content: [{ type: "text", text: "line\n".repeat(50) }],
  });
  assert.deepEqual(result, { content: [{ type: "text", text: "short [rtok expand abc]" }] });
});

test("unchanged or empty filter output keeps the original", posix, async () => {
  const same = load(fakeRtok("echo"));
  assert.equal(
    await same.on.tool_result({ toolName: "bash", content: [{ text: "small\n" }] }),
    undefined,
  );
  assert.equal(await same.on.tool_result({ toolName: "bash", content: [] }), undefined);
  const empty = load(fakeRtok(""));
  assert.equal(
    await empty.on.tool_result({ toolName: "bash", content: [{ text: "x" }] }),
    undefined,
  );
});
