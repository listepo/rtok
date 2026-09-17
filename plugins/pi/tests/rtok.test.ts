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
function load(body: string | null, withoutSendMessage = false) {
  fakeRtok(body);
  const on: Record<string, Handler> = {};
  const entries: [string, string][] = [];
  const messages: any[] = [];
  const pi: any = {
    on: (name: string, fn: Handler) => (on[name] = fn),
    appendEntry: (kind: string, text: string) => entries.push([kind, text]),
  };
  if (!withoutSendMessage) {
    pi.sendMessage = (message: any) => messages.push(message);
  }
  extension(pi);
  return { on, entries, messages };
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
  const { on, entries, messages } = load(null);
  const event = { toolName: "bash", input: { command: "ls" } };
  await on.tool_call(event);
  assert.equal(event.input.command, "ls", "the command runs unchanged");
  assert.equal(messages.length, 1, "the hint reaches the model via sendMessage");
  assert.match(String(messages[0]?.content ?? ""), /ketch install listepo\/rtok/);
  assert.equal(entries.length, 0, "TUI-only appendEntry stays unused when sendMessage exists");
  await on.tool_call({ toolName: "bash", input: { command: "pwd" } });
  assert.equal(messages.length, 1, "once per session");
  const result = await on.tool_result({ toolName: "bash", content: [{ text: "big" }] });
  assert.equal(result, undefined, "the result passes through");
});

test("without sendMessage the hint falls back to appendEntry once", async () => {
  const { on, entries, messages } = load(null, true);
  await on.tool_call({ toolName: "bash", input: { command: "ls" } });
  await on.tool_call({ toolName: "bash", input: { command: "pwd" } });
  assert.equal(messages.length, 0);
  assert.equal(entries.length, 1);
  assert.match(entries[0][1], /ketch install listepo\/rtok/);
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
