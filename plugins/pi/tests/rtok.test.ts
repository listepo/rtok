// Unit test of the pi extension (T47.3): the extension talks to `rtok` on PATH, so each case
// puts a fake `rtok` first on PATH (tests/node/fake-rtok.ts) and drives the two handlers through
// a stub `pi`. Lives outside `extensions/` so pi never loads it.
import { fakeHangingRtok, fakeRtok } from "../../../tests/node/fake-rtok.ts";
import extension from "../extensions/rtok.ts";

/** `filter` prints `out`; `"echo"` prints stdin back. */
const filterPrints = (out: string) =>
  out === "echo" ? "process.stdout.write(input);" : `process.stdout.write(${JSON.stringify(out)});`;

type Handler = (event: any) => Promise<any>;

/** Load the extension against a stub `pi`, with `rtok` as `fakeRtok(body)` sets it up.
 * `extra` adds host-specific API members (omp injects its SDK as `pi.pi`). */
function load(body: string | null, extra: Record<string, unknown> = {}) {
  fakeRtok(body);
  const on: Record<string, Handler> = {};
  const entries: [string, string][] = [];
  const tools: any[] = [];
  extension({
    on: (name: string, fn: Handler) => (on[name] = fn),
    appendEntry: (kind: string, text: string) => entries.push([kind, text]),
    registerTool: (t: any) => tools.push(t),
    ...extra,
  });
  return { on, entries, tools };
}

test("bash calls are rewritten to one quoted `rtok run --`", async () => {
  const { on } = load(filterPrints("x"));
  const event = { toolName: "bash", input: { command: "echo it's" } };
  await on.tool_call(event);
  expect(event.input.command).toBe(`rtok run -- 'echo it'"'"'s'`);
  await on.tool_call(event);
  expect(event.input.command, "never wrapped twice").toBe(`rtok run -- 'echo it'"'"'s'`);
});

// omp applies a revised input only when the handler returns it (T92.1).
test("the bash rewrite is also returned as `input`", async () => {
  const { on } = load(filterPrints("x"));
  const event = { toolName: "bash", input: { command: "ls" } };
  expect(await on.tool_call(event)).toEqual({ input: { command: "rtok run -- 'ls'" } });
});

test("other tools are left alone", async () => {
  const { on } = load(filterPrints("x"));
  const event = { toolName: "write", input: { command: "echo hi" } };
  await on.tool_call(event);
  expect(event.input.command).toBe("echo hi");
  expect(await on.tool_result({ toolName: "write", content: [{ text: "a" }] })).toBeUndefined();
});

test("a large read result is replaced and carries an expand trailer", async () => {
  const { on } = load(
    `if (args.join(" ").indexOf("filter --stdin --cmd read src/lib.rs") < 0) process.exit(9);
process.stdout.write("short [rtok expand abc]");`,
  );
  const result = await on.tool_result({
    toolName: "read",
    input: { path: "src/lib.rs" },
    content: [{ type: "text", text: "line\n".repeat(80) }],
  });
  expect(result).toEqual({ content: [{ type: "text", text: "short [rtok expand abc]" }] });
});

test("a small read result stays byte-identical", async () => {
  const { on } = load(filterPrints("echo"));
  expect(
    await on.tool_result({
      toolName: "read",
      input: { path: "tiny.rs" },
      content: [{ text: "small\n" }],
    }),
  ).toBeUndefined();
});

test("a spawn failure on read returns the original", async () => {
  const { on, entries } = load(null);
  expect(
    await on.tool_result({
      toolName: "read",
      input: { path: "x.rs" },
      content: [{ text: "whole file" }],
    }),
  ).toBeUndefined();
  expect(entries).toHaveLength(1);
  expect(entries[0][1]).toMatch(/ketch install listepo\/rtok/);
});

test("missing rtok fails open and names ketch", async () => {
  const { on, entries } = load(null);
  const event = { toolName: "bash", input: { command: "ls" } };
  await on.tool_call(event);
  expect(event.input.command, "the command runs unchanged").toBe("ls");
  expect(entries).toHaveLength(1);
  expect(entries[0][1]).toMatch(/ketch install listepo\/rtok/);
  const result = await on.tool_result({ toolName: "bash", content: [{ text: "big" }] });
  expect(result, "the result passes through").toBeUndefined();
});

test("a shorter filter result replaces the bash output", async () => {
  const { on } = load(filterPrints("short [rtok expand abc]"));
  const result = await on.tool_result({
    toolName: "bash",
    content: [{ type: "text", text: "line\n".repeat(50) }],
  });
  expect(result).toEqual({ content: [{ type: "text", text: "short [rtok expand abc]" }] });
});

test("unchanged or empty filter output keeps the original", async () => {
  const same = load(filterPrints("echo"));
  expect(
    await same.on.tool_result({ toolName: "bash", content: [{ text: "small\n" }] }),
  ).toBeUndefined();
  expect(await same.on.tool_result({ toolName: "bash", content: [] })).toBeUndefined();
  const empty = load(filterPrints(""));
  expect(
    await empty.on.tool_result({ toolName: "bash", content: [{ text: "x" }] }),
  ).toBeUndefined();
});

test("a wedged rtok times out and tool_result keeps the original", async () => {
  fakeHangingRtok();
  const on: Record<string, Handler> = {};
  extension({ on: (name: string, fn: Handler) => (on[name] = fn) });
  const start = Date.now();
  const result = await on.tool_result({
    toolName: "bash",
    content: [{ type: "text", text: "original output" }],
  });
  expect(result, "the result passes through unchanged").toBeUndefined();
  // Two sequential spawns (PostToolUse hook, then filter) each time out at 5 s.
  expect(Date.now() - start).toBeLessThan(20_000);
}, 30_000);

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
  expect(out, "the handler returns a replacement array").toBeTruthy();
  expect(out.messages).toMatchInlineSnapshot(`
    [
      {
        "content": [
          {
            "text": "prompt",
            "type": "text",
          },
        ],
        "role": "user",
      },
      {
        "content": [
          {
            "text": "working",
            "type": "text",
          },
        ],
        "role": "assistant",
      },
      {
        "content": [
          {
            "text": "[archived fake-id: 1 lines · 1 tokens · expand(fake-id)]",
            "type": "text",
          },
        ],
        "isError": false,
        "role": "toolResult",
        "toolCallId": "call_1",
        "toolName": "bash",
      },
    ]
  `);
});

test("context: an untouched array keeps pi's exact object", async () => {
  const { on } = load(ARCHIVE_REWRITES);
  // Small result: rtok echoes the input bytes, so the handler changes nothing.
  expect(await on.context({ messages: piArray(false) })).toBeUndefined();
});

test("context: spawn failure or garbage output keeps the array (fail open)", async () => {
  const missing = load(null);
  expect(await missing.on.context({ messages: piArray(true) })).toBeUndefined();
  const garbage = load(filterPrints("not json {{{"));
  expect(await garbage.on.context({ messages: piArray(true) })).toBeUndefined();
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
  const ret = await on.session_before_compact({ reason: "threshold" }, { sessionId: "p1" });
  expect(ret, "must not replace the host summary").toBeUndefined();
});

test("after compact, the next context injects the checkpoint", async () => {
  const { on } = load(COMPACT);
  expect(await on.session_before_compact({ reason: "auto" }, { sessionId: "p1" })).toBeUndefined();
  await on.session_compact({}, { sessionId: "p1" });
  const messages = piArray(false);
  const out = await on.context({ messages });
  expect(out, "restore must return a replacement array").toBeTruthy();
  expect(out.messages.at(-1)).toEqual({ role: "user", content: [{ type: "text", text: CKPT }] });
  expect(await on.context({ messages: piArray(false) }), "once").toBeUndefined();
});

test("compaction without rtok fails open", async () => {
  const { on } = load(null);
  expect(
    await on.session_before_compact({ reason: "overflow" }, { sessionId: "p1" }),
  ).toBeUndefined();
  await on.session_compact({}, { sessionId: "p1" });
  expect(await on.context({ messages: piArray(false) })).toBeUndefined();
});

const GUARD_DENY = `
if (args.includes("guard")) {
  process.stdout.write(JSON.stringify({allow:false, reason:"duplicate; rtok expand abc"}));
} else {
  process.stdout.write("x");
}
`;

/** An allowed bash call: not blocked, and the rewrite is returned (omp applies only that). */
const WRAPPED_LS = { input: { command: "rtok run -- 'ls'" } };

test("guard deny with a reason blocks the call", async () => {
  const { on } = load(GUARD_DENY);
  const event = { toolName: "bash", input: { command: "ls" } };
  const ret = await on.tool_call(event, { sessionId: "s1" });
  expect(ret).toEqual({ block: true, reason: "duplicate; rtok expand abc" });
  expect(event.input.command, "must not wrap a denied call").toBe("ls");
});

test("guard allow still wraps bash", async () => {
  const { on } = load(`
    if (args.includes("guard")) process.stdout.write(JSON.stringify({allow:true}));
    else process.stdout.write("x");
  `);
  const event = { toolName: "bash", input: { command: "ls" } };
  expect(await on.tool_call(event, { sessionId: "s1" })).toEqual(WRAPPED_LS);
  expect(event.input.command).toBe("rtok run -- 'ls'");
});

test("guard deny without a reason fails open", async () => {
  const { on } = load(`
    if (args.includes("guard")) process.stdout.write(JSON.stringify({allow:false}));
    else process.stdout.write("x");
  `);
  const event = { toolName: "bash", input: { command: "ls" } };
  expect(await on.tool_call(event, { sessionId: "s1" })).toEqual(WRAPPED_LS);
  expect(event.input.command).toBe("rtok run -- 'ls'");
});

test("unparsable guard output fails open", async () => {
  const { on } = load(filterPrints("x"));
  const event = { toolName: "bash", input: { command: "ls" } };
  expect(await on.tool_call(event, { sessionId: "s1" })).toEqual(WRAPPED_LS);
  expect(event.input.command).toMatch(/rtok run/);
});

test("tools stay unregistered until setup.pi.tools is true", async () => {
  const off = load('process.stdout.write("false");');
  await off.on.session_start({});
  expect(off.tools).toHaveLength(0);
  const missing = load(null);
  await missing.on.session_start({});
  expect(missing.tools, "missing rtok fails open").toHaveLength(0);
});

test("each registered tool is one mcp --call", async () => {
  const { on, tools } = load(`
    if (args.includes("config")) process.stdout.write("true");
    else process.stdout.write(args.join(" "));
  `);
  await on.session_start({});
  const calls = [];
  for (const t of tools) calls.push((await t.execute("id1", { q: 1 })).content[0].text);
  expect(calls).toMatchInlineSnapshot(`
    [
      "mcp --call read --json {"q":1}",
      "mcp --call search --json {"q":1}",
      "mcp --call tree --json {"q":1}",
      "mcp --call symbol --json {"q":1}",
      "mcp --call callers --json {"q":1}",
      "mcp --call expand --json {"q":1}",
      "mcp --call mem_search --json {"q":1}",
      "mcp --call mem_get --json {"q":1}",
    ]
  `);
});

test("under omp (`pi.pi` present) tools stay unregistered even when setup.pi.tools is true", async () => {
  const { on, tools } = load('process.stdout.write("true");', { pi: { VERSION: "18.1.14" } });
  await on.session_start({});
  expect(tools, "omp's native MCP owns these tools (D21)").toHaveLength(0);
});

test("registered tool execute fails open when rtok is missing", async () => {
  const { on, tools } = load('process.stdout.write("true");');
  await on.session_start({});
  expect(tools).toHaveLength(8);
  fakeRtok(null);
  const out = await tools[0].execute("id1", { path: "a.rs" });
  expect(out.content[0].text).toMatch(/ketch install listepo\/rtok/);
});
