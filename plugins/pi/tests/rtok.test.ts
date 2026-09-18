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

/** The `archive rewrite --stdin` fake: rewrites the first large toolResult to a pointer. */
const ARCHIVE_REWRITES = `
if (args.includes("archive")) {
  const messages = JSON.parse(input);
  for (const m of messages) {
    if (m.role === "toolResult" && typeof m.content?.[0]?.text === "string" && m.content[0].text.length > 20) {
      m.content = [{ type: "text", text: "[archived fake-id: 1 lines · 1 tokens · expand(fake-id)]" }];
      break;
    }
  }
  process.stdout.write(JSON.stringify(messages));
} else {
  process.stdout.write(input);
}
`;

/** A pi `context` message array, real-session shape (toolResult / camelCase toolCallId). */
function piArray(large: boolean) {
  return [
    { role: "user", content: [{ type: "text", text: "prompt" }] },
    { role: "assistant", content: [{ type: "text", text: "working" }] },
    {
      role: "toolResult",
      toolCallId: "call_1",
      toolName: "bash",
      content: [{ type: "text", text: large ? "line\n".repeat(200) : "small" }],
      isError: false,
    },
  ];
}

test("context: a large message array comes back shortened with an expand pointer", async () => {
  const { on } = load(ARCHIVE_REWRITES);
  const out = await on.context({ messages: piArray(true) });
  assert.ok(out, "the handler returns a replacement array");
  const result = out.messages.find((m: any) => m.role === "toolResult");
  assert.match(result.content[0].text, /\[archived fake-id/);
  assert.match(result.content[0].text, /expand\(fake-id\)/);
});

test("context: an untouched array keeps pi's exact object", async () => {
  const { on } = load(ARCHIVE_REWRITES);
  // Small result: rtok echoes the input bytes, so the handler changes nothing.
  assert.equal(await on.context({ messages: piArray(false) }), undefined);
});

test("context: spawn failure or garbage output keeps the array (fail open)", async () => {
  const missing = load(null);
  assert.equal(await missing.on.context({ messages: piArray(true) }), undefined);
  const garbage = load(filterPrints("not json {{{"));
  assert.equal(await garbage.on.context({ messages: piArray(true) }), undefined);
});

const CKPT = "checkpoint\n- edit the three files\n";
const COMPACT = `
if (args.includes("hook") && args.includes("PostCompact")) {
  process.stdout.write(JSON.stringify({hookSpecificOutput:{additionalContext:${JSON.stringify(CKPT)}}}));
} else if (args.includes("hook")) {
  process.stdout.write("{}");
} else if (args.includes("archive")) {
  process.stdout.write(input);
} else {
  process.stdout.write("{}");
}
`;

test("session_before_compact calls PreCompact --host pi and returns nothing", async () => {
  const { on } = load(`
    if (args.join(" ") !== "hook PreCompact --host pi") process.exit(9);
    process.stdout.write("{}");
  `);
  const ret = await on.session_before_compact(
    { reason: "threshold" },
    { sessionId: "p1" },
  );
  assert.equal(ret, undefined, "must not replace the host summary");
});

test("after compact, the next context injects the checkpoint", async () => {
  const { on } = load(COMPACT);
  assert.equal(
    await on.session_before_compact({ reason: "auto" }, { sessionId: "p1" }),
    undefined,
  );
  await on.session_compact({}, { sessionId: "p1" });
  const messages = piArray(false);
  const out = await on.context({ messages });
  assert.ok(out, "restore must return a replacement array");
  assert.equal(out.messages.at(-1).content[0].text, CKPT);
  assert.equal(await on.context({ messages: piArray(false) }), undefined, "once");
});

test("compaction without rtok fails open", async () => {
  const { on } = load(null);
  assert.equal(
    await on.session_before_compact({ reason: "overflow" }, { sessionId: "p1" }),
    undefined,
  );
  await on.session_compact({}, { sessionId: "p1" });
  assert.equal(await on.context({ messages: piArray(false) }), undefined);
});

const GUARD_DENY = `
if (args.includes("guard")) {
  process.stdout.write(JSON.stringify({allow:false, reason:"duplicate; rtok expand abc"}));
} else {
  process.stdout.write("x");
}
`;

test("guard deny with a reason blocks the call", async () => {
  const { on } = load(GUARD_DENY);
  const event = { toolName: "bash", input: { command: "ls" } };
  const ret = await on.tool_call(event, { sessionId: "s1" });
  assert.deepEqual(ret, { block: true, reason: "duplicate; rtok expand abc" });
  assert.equal(event.input.command, "ls", "must not wrap a denied call");
});

test("guard allow still wraps bash", async () => {
  const { on } = load(`
    if (args.includes("guard")) process.stdout.write(JSON.stringify({allow:true}));
    else process.stdout.write("x");
  `);
  const event = { toolName: "bash", input: { command: "ls" } };
  assert.equal(await on.tool_call(event, { sessionId: "s1" }), undefined);
  assert.equal(event.input.command, "rtok run -- 'ls'");
});

test("guard deny without a reason fails open", async () => {
  const { on } = load(`
    if (args.includes("guard")) process.stdout.write(JSON.stringify({allow:false}));
    else process.stdout.write("x");
  `);
  const event = { toolName: "bash", input: { command: "ls" } };
  assert.equal(await on.tool_call(event, { sessionId: "s1" }), undefined);
  assert.equal(event.input.command, "rtok run -- 'ls'");
});

test("unparsable guard output fails open", async () => {
  const { on } = load(filterPrints("x"));
  const event = { toolName: "bash", input: { command: "ls" } };
  assert.equal(await on.tool_call(event, { sessionId: "s1" }), undefined);
  assert.match(event.input.command, /rtok run/);
});
