import { fakeRtok } from "../../tests/node/fake-rtok.ts";
import { createPlugin, filterStdin, guardCheck, hookStdin } from "./rtok.ts";

test("replaces bash output via the injected filter", async () => {
  const plugin = await createPlugin((cmd, stdin) => {
    expect(cmd).toBe("git status");
    expect(stdin).toMatch(/Changes not staged/);
    return "On branch main\nmodified:   src/lib.rs\n";
  })();
  const output = {
    output: "On branch main\nChanges not staged for commit:\n\tmodified:   src/lib.rs\n",
  };
  await plugin["tool.execute.after"]({ tool: "bash", args: { command: "git status" } }, output);
  expect(output.output).toBe("On branch main\nmodified:   src/lib.rs\n");
});

test("replaces skill output via the injected filter", async () => {
  const plugin = await createPlugin((cmd, stdin) => {
    expect(cmd).toBe("skill nx-workspace");
    expect(stdin).toMatch(/Nx Workspace/);
    return "# Nx Workspace Exploration\n";
  })();
  const output = {
    output: '<skill_content name="nx-workspace">\n# Nx Workspace Exploration\nbody\n',
  };
  await plugin["tool.execute.after"]({ tool: "skill", args: { name: "nx-workspace" } }, output);
  expect(output.output).toBe("# Nx Workspace Exploration\n");
});

test("filters a 3000-line skill body", async () => {
  const body = Array.from({ length: 3000 }, (_, i) => `line ${i}`).join("\n");
  const plugin = await createPlugin((cmd, stdin) => {
    expect(cmd).toBe("skill demo");
    expect(stdin.split("\n")).toHaveLength(3000);
    return "head\n";
  })();
  const output = { output: body };
  await plugin["tool.execute.after"]({ tool: "skill", args: { name: "demo" } }, output);
  expect(output.output).toBe("head\n");
});

test("small skill body still goes through the filter", async () => {
  const plugin = await createPlugin((cmd, stdin) => {
    expect(cmd).toBe("skill tiny");
    return stdin;
  })();
  const output = { output: "# Tiny\n" };
  await plugin["tool.execute.after"]({ tool: "skill", args: { name: "tiny" } }, output);
  expect(output.output).toBe("# Tiny\n");
});

test("leaves non-bash tools unchanged", async () => {
  const plugin = await createPlugin(() => {
    throw new Error("filter must not run");
  })();
  const output = { output: "fn main() {}" };
  await plugin["tool.execute.after"]({ tool: "read" }, output);
  expect(output.output).toBe("fn main() {}");
});

test("filterStdin passes the command and stdin to `rtok filter`", () => {
  fakeRtok(
    `if (args.join(" ") !== "filter --stdin --cmd git status") process.exit(9);\n` +
      `process.stdout.write(input.toUpperCase());`,
  );
  expect(filterStdin("git status", "on branch")).toBe("ON BRANCH");
});

test("filterStdin archives skill stdin", () => {
  fakeRtok(
    `if (args.join(" ") !== "filter --stdin --cmd skill nx --archive") process.exit(9);\n` +
      `process.stdout.write("cut");`,
  );
  expect(filterStdin("skill nx", "body")).toBe("cut");
});

test("filterStdin fails open on a non-zero exit", () => {
  fakeRtok(`process.stdout.write("partial"); process.exit(1);`);
  expect(filterStdin("ls", "original")).toBe("original");
});

test("missing rtok fails open and names ketch once", () => {
  fakeRtok(null);
  const errors: string[] = [];
  vi.spyOn(console, "error").mockImplementation((msg: string) => errors.push(msg));
  expect(filterStdin("ls", "original")).toBe("original");
  expect(filterStdin("ls", "again")).toBe("again");
  expect(errors, "the hint is said once per process").toHaveLength(1);
  expect(errors[0]).toMatch(/ketch install listepo\/rtok/);
});

const CKPT = "checkpoint\n- edit the three files\n";

test("hookStdin calls rtok hook with --host opencode", () => {
  fakeRtok(
    `if (args.join(" ") !== "hook PreCompact --host opencode") process.exit(9);\n` +
      `process.stdout.write(JSON.stringify({hookSpecificOutput:{additionalContext:${JSON.stringify(CKPT)}}}));`,
  );
  expect(hookStdin("PreCompact", "{}")).toBe(CKPT);
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
  expect(output.context).toEqual(["host", CKPT]);
  expect(output.prompt).toBeUndefined();
  expect(calls).toMatchInlineSnapshot(`
    [
      "PreCompact {"hook_event_name":"PreCompact","session_id":"s1","trigger":"auto"}",
      "SessionStart {"hook_event_name":"SessionStart","session_id":"s1","source":"compact"}",
    ]
  `);
});

test("next system transform injects the compact restore once", async () => {
  const plugin = await createPlugin(
    () => "",
    (event) => (event === "SessionStart" || event === "PostCompact" ? CKPT : ""),
  )();
  await plugin["experimental.session.compacting"]({ sessionID: "s1" }, { context: [] });
  const sys = { system: ["base"] };
  await plugin["experimental.chat.system.transform"]({ sessionID: "s1" }, sys);
  expect(sys.system).toEqual(["base", CKPT]);
  const again = { system: ["base"] };
  await plugin["experimental.chat.system.transform"]({ sessionID: "s1" }, again);
  expect(again.system).toEqual(["base"]);
});

test("missing rtok compacting fails open", async () => {
  fakeRtok(null);
  const plugin = await createPlugin()();
  const output: { context: string[]; prompt?: string } = { context: ["host"] };
  await plugin["experimental.session.compacting"]({ sessionID: "s" }, output);
  expect(output.context).toEqual(["host"]);
  expect(output.prompt).toBeUndefined();
});

test("guardCheck denies with a reason", () => {
  fakeRtok(
    `if (!args.includes("guard")) process.exit(9);
     process.stdout.write(JSON.stringify({allow:false, reason:"duplicate; rtok expand abc"}));`,
  );
  expect(guardCheck("bash", { command: "ls" }, "s")).toEqual({
    allow: false,
    reason: "duplicate; rtok expand abc",
  });
});

test("guardCheck fails open on a non-zero exit", () => {
  fakeRtok(`process.stdout.write("partial"); process.exit(1);`);
  expect(guardCheck("bash", { command: "ls" }, "s")).toEqual({ allow: true });
});

test("guardCheck fails open when rtok is missing", () => {
  fakeRtok(null);
  expect(guardCheck("bash", { command: "ls" }, "s")).toEqual({ allow: true });
});

test("before throws the deny reason and stays silent without one", async () => {
  const deny = await createPlugin(
    () => {
      throw new Error("filter must not run");
    },
    () => "",
    () => ({ allow: false, reason: "duplicate; rtok expand abc" }),
  )();
  await expect(
    deny["tool.execute.before"]({ tool: "bash", sessionID: "s" }, { args: { command: "ls" } }),
  ).rejects.toThrow(/duplicate; rtok expand abc/);
  const silent = await createPlugin(
    () => {
      throw new Error("filter must not run");
    },
    () => "",
    () => ({ allow: false }),
  )();
  await silent["tool.execute.before"](
    { tool: "bash", sessionID: "s" },
    { args: { command: "ls" } },
  );
});

test("before allow does not throw", async () => {
  const plugin = await createPlugin(
    () => {
      throw new Error("filter must not run");
    },
    () => "",
    () => ({ allow: true }),
  )();
  await plugin["tool.execute.before"](
    { tool: "read", sessionID: "s" },
    { args: { filePath: "a.rs" } },
  );
});
