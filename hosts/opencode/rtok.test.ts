import assert from "node:assert/strict";
import { test } from "node:test";
import { createPlugin } from "./rtok.ts";

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

test("leaves non-bash tools unchanged", async () => {
  const plugin = await createPlugin(() => {
    throw new Error("filter must not run");
  })();
  const output = { output: "fn main() {}" };
  await plugin["tool.execute.after"]({ tool: "read" }, output);
  assert.equal(output.output, "fn main() {}");
});
