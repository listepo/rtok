// Unit test of the pi extension (T47.3): the extension talks to `rtok` on PATH, so each case
// puts a fake `rtok` first on PATH (tests/node/fake-rtok.ts) and drives the two handlers through
// a stub `pi`. Lives outside `extensions/` so pi never loads it.
import assert from "node:assert/strict";
import { test } from "node:test";
import { fakeRtok } from "../../../tests/node/fake-rtok.ts";
import extension from "../extensions/rtok.ts";

/** `filter` prints `out`; `"echo"` prints stdin back. */
const filterPrints = (out: string) =>
  out === "echo" ? "process.stdout.write(input);" : `process.stdout.write(${JSON.stringify(out)});`;

type Handler = (event: any) => Promise<any>;

/** Load the extension against a stub `pi`, with `rtok` as `fakeRtok(body)` sets it up. */
function load(body: string | null) {
  fakeRtok(body);
  const on: Record<string, Handler> = {};
  const entries: [string, string][] = [];
  extension({
    on: (name: string, fn: Handler) => (on[name] = fn),
    appendEntry: (kind: string, text: string) => entries.push([kind, text]),
  });
  return { on, entries };
}

test("bash calls are rewritten to one quoted `rtok run --`", async () => {
  const { on } = load(filterPrints("x"));
  const event = { toolName: "bash", input: { command: "echo it's" } };
  await on.tool_call(event);
  assert.equal(event.input.command, `rtok run -- 'echo it'"'"'s'`);
  await on.tool_call(event);
  assert.equal(event.input.command, `rtok run -- 'echo it'"'"'s'`, "never wrapped twice");
});

test("other tools are left alone", async () => {
  const { on } = load(filterPrints("x"));
  const event = { toolName: "read", input: { command: "echo hi" } };
  await on.tool_call(event);
  assert.equal(event.input.command, "echo hi");
  assert.equal(await on.tool_result({ toolName: "read", content: [{ text: "a" }] }), undefined);
});

test("missing rtok fails open and names ketch", async () => {
  const { on, entries } = load(null);
  const event = { toolName: "bash", input: { command: "ls" } };
  await on.tool_call(event);
  assert.equal(event.input.command, "ls", "the command runs unchanged");
  assert.equal(entries.length, 1);
  assert.match(entries[0][1], /ketch install listepo\/rtok/);
  const result = await on.tool_result({ toolName: "bash", content: [{ text: "big" }] });
  assert.equal(result, undefined, "the result passes through");
});

test("a shorter filter result replaces the bash output", async () => {
  const { on } = load(filterPrints("short [rtok expand abc]"));
  const result = await on.tool_result({
    toolName: "bash",
    content: [{ type: "text", text: "line\n".repeat(50) }],
  });
  assert.deepEqual(result, { content: [{ type: "text", text: "short [rtok expand abc]" }] });
});

test("unchanged or empty filter output keeps the original", async () => {
  const same = load(filterPrints("echo"));
  assert.equal(
    await same.on.tool_result({ toolName: "bash", content: [{ text: "small\n" }] }),
    undefined,
  );
  assert.equal(await same.on.tool_result({ toolName: "bash", content: [] }), undefined);
  const empty = load(filterPrints(""));
  assert.equal(
    await empty.on.tool_result({ toolName: "bash", content: [{ text: "x" }] }),
    undefined,
  );
});
