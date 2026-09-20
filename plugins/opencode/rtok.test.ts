import assert from "node:assert/strict";
import { test } from "node:test";
import { fakeRtok } from "../../tests/node/fake-rtok.ts";
import { createPlugin, filterStdin, guardCheck, hookStdin } from "./rtok.ts";

test("replaces bash output via the injected filter", async () => {
  const plugin = await createPlugin((cmd, stdin) => {
    assert.equal(cmd, "git status");
    assert.match(stdin, /Changes not staged/);
    return "On branch main\nmodified:   src/lib.rs\n";
  })();
  const output = {
    output:
      "On branch main\nChanges not staged for commit:\n\tmodified:   src/lib.rs\n",
  };
  await plugin["tool.execute.after"](
    { tool: "bash", args: { command: "git status" } },
    output,
  );
  assert.equal(output.output, "On branch main\nmodified:   src/lib.rs\n");
});

test("replaces skill output via the injected filter", async () => {
  const plugin = await createPlugin((cmd, stdin) => {
    assert.equal(cmd, "skill nx-workspace");
    assert.match(stdin, /Nx Workspace/);
    return "# Nx Workspace Exploration\n";
  })();
  const output = {
    output: "<skill_content name=\"nx-workspace\">\n# Nx Workspace Exploration\nbody\n",
  };
  await plugin["tool.execute.after"](
    { tool: "skill", args: { name: "nx-workspace" } },
    output,
  );
  assert.equal(output.output, "# Nx Workspace Exploration\n");
});

test("filters a 3000-line skill body", async () => {
  const body = Array.from({ length: 3000 }, (_, i) => `line ${i}`).join("\n");
  const plugin = await createPlugin((cmd, stdin) => {
    assert.equal(cmd, "skill demo");
    assert.equal(stdin.split("\n").length, 3000);
    return "head\n";
  })();
  const output = { output: body };
  await plugin["tool.execute.after"]({ tool: "skill", args: { name: "demo" } }, output);
  assert.equal(output.output, "head\n");
});

test("small skill body still goes through the filter", async () => {
  const plugin = await createPlugin((cmd, stdin) => {
    assert.equal(cmd, "skill tiny");
    return stdin;
  })();
  const output = { output: "# Tiny\n" };
  await plugin["tool.execute.after"]({ tool: "skill", args: { name: "tiny" } }, output);
  assert.equal(output.output, "# Tiny\n");
});

test("leaves non-bash tools unchanged", async () => {
  const plugin = await createPlugin(() => {
    throw new Error("filter must not run");
  })();
  const output = { output: "fn main() {}" };
  await plugin["tool.execute.after"]({ tool: "read" }, output);
  assert.equal(output.output, "fn main() {}");
});

test("filterStdin passes the command and stdin to `rtok filter`", () => {
  fakeRtok(
    `if (args.join(" ") !== "filter --stdin --cmd git status") process.exit(9);\n` +
      `process.stdout.write(input.toUpperCase());`,
  );
  assert.equal(filterStdin("git status", "on branch"), "ON BRANCH");
});

test("filterStdin archives skill stdin", () => {
  fakeRtok(
    `if (args.join(" ") !== "filter --stdin --cmd skill nx --archive") process.exit(9);\n` +
      `process.stdout.write("cut");`,
  );
  assert.equal(filterStdin("skill nx", "body"), "cut");
});

test("filterStdin fails open on a non-zero exit", () => {
  fakeRtok(`process.stdout.write("partial"); process.exit(1);`);
  assert.equal(filterStdin("ls", "original"), "original");
});

test("missing rtok fails open and names ketch once", (t) => {
  fakeRtok(null);
  const errors: string[] = [];
  t.mock.method(console, "error", (msg: string) => errors.push(msg));
  assert.equal(filterStdin("ls", "original"), "original");
  assert.equal(filterStdin("ls", "again"), "again");
  assert.equal(errors.length, 1, "the hint is said once per process");
  assert.match(errors[0], /ketch install listepo\/rtok/);
});

const CKPT = "checkpoint\n- edit the three files\n";

test("hookStdin calls rtok hook with --host opencode", () => {
  fakeRtok(
    `if (args.join(" ") !== "hook PreCompact --host opencode") process.exit(9);\n` +
      `process.stdout.write(JSON.stringify({hookSpecificOutput:{additionalContext:${JSON.stringify(CKPT)}}}));`,
  );
  assert.equal(hookStdin("PreCompact", "{}"), CKPT);
});

test("compacting appends the checkpoint and never replaces the prompt", async () => {
  const calls: string[] = [];
  const plugin = await createPlugin(
    () => {
      throw new Error("filter must not run");
    },
    (event, stdin) => {
      calls.push(`${event} ${stdin}`);
      return event === "SessionStart" ? CKPT : "";
    },
  )();
  const output: { context: string[]; prompt?: string } = { context: ["host"] };
  await plugin["experimental.session.compacting"]({ sessionID: "s1" }, output);
  assert.deepEqual(output.context, ["host", CKPT]);
  assert.equal(output.prompt, undefined);
  assert.ok(calls.some((c) => c.startsWith("PreCompact ") && c.includes('"session_id":"s1"')));
  assert.ok(calls.some((c) => c.startsWith("SessionStart ") && c.includes('"source":"compact"')));
});

test("next system transform injects the compact restore once", async () => {
  const plugin = await createPlugin(
    () => "",
    (event) => (event === "SessionStart" || event === "PostCompact" ? CKPT : ""),
  )();
  await plugin["experimental.session.compacting"](
    { sessionID: "s1" },
    { context: [] },
  );
  const sys = { system: ["base"] };
  await plugin["experimental.chat.system.transform"]({ sessionID: "s1" }, sys);
  assert.deepEqual(sys.system, ["base", CKPT]);
  const again = { system: ["base"] };
  await plugin["experimental.chat.system.transform"]({ sessionID: "s1" }, again);
  assert.deepEqual(again.system, ["base"]);
});

test("missing rtok compacting fails open", async () => {
  fakeRtok(null);
  const plugin = await createPlugin()();
  const output: { context: string[]; prompt?: string } = { context: ["host"] };
  await plugin["experimental.session.compacting"]({ sessionID: "s" }, output);
  assert.deepEqual(output.context, ["host"]);
  assert.equal(output.prompt, undefined);
});

test("guardCheck denies with a reason", () => {
  fakeRtok(
    `if (!args.includes("guard")) process.exit(9);
     process.stdout.write(JSON.stringify({allow:false, reason:"duplicate; rtok expand abc"}));`,
  );
  assert.deepEqual(guardCheck("bash", { command: "ls" }, "s"), {
    allow: false,
    reason: "duplicate; rtok expand abc",
  });
});

test("guardCheck fails open on a non-zero exit", () => {
  fakeRtok(`process.stdout.write("partial"); process.exit(1);`);
  assert.deepEqual(guardCheck("bash", { command: "ls" }, "s"), { allow: true });
});

test("guardCheck fails open when rtok is missing", () => {
  fakeRtok(null);
  assert.deepEqual(guardCheck("bash", { command: "ls" }, "s"), { allow: true });
});

test("before throws the deny reason and stays silent without one", async () => {
  const deny = await createPlugin(
    () => {
      throw new Error("filter must not run");
    },
    () => "",
    () => ({ allow: false, reason: "duplicate; rtok expand abc" }),
  )();
  await assert.rejects(
    () => deny["tool.execute.before"]({ tool: "bash", sessionID: "s" }, { args: { command: "ls" } }),
    /duplicate; rtok expand abc/,
  );
  const silent = await createPlugin(
    () => {
      throw new Error("filter must not run");
    },
    () => "",
    () => ({ allow: false }),
  )();
  await silent["tool.execute.before"]({ tool: "bash", sessionID: "s" }, { args: { command: "ls" } });
});

test("before allow does not throw", async () => {
  const plugin = await createPlugin(
    () => {
      throw new Error("filter must not run");
    },
    () => "",
    () => ({ allow: true }),
  )();
  await plugin["tool.execute.before"]({ tool: "read", sessionID: "s" }, { args: { filePath: "a.rs" } });
});
