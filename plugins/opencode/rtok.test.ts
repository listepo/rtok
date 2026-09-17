import assert from "node:assert/strict";
import { test } from "node:test";
import { chmodSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createPlugin, filterStdin } from "./rtok.ts";

const posix = { skip: process.platform === "win32" ? "fake rtok is a sh script" : false };

/** PATH = a dir holding `rtok` with `script` as its body (none when null), plus /bin for sh. */
function withRtok(script: string | null) {
  const dir = mkdtempSync(join(tmpdir(), "rtok-opencode-"));
  if (script !== null) {
    writeFileSync(join(dir, "rtok"), `#!/bin/sh\n${script}\n`);
    chmodSync(join(dir, "rtok"), 0o755);
  }
  process.env.PATH = `${dir}:/bin:/usr/bin`;
}

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

test("filterStdin passes the command and stdin to `rtok filter`", posix, () => {
  withRtok(`[ "$1 $2 $3 $4" = "filter --stdin --cmd git status" ] || exit 9\ntr a-z A-Z`);
  assert.equal(filterStdin("git status", "on branch"), "ON BRANCH");
});

test("filterStdin fails open on a non-zero exit", posix, () => {
  withRtok("echo partial; exit 1");
  assert.equal(filterStdin("ls", "original"), "original");
});

test("missing rtok fails open and names ketch once", posix, (t) => {
  withRtok(null);
  const errors: string[] = [];
  t.mock.method(console, "error", (msg: string) => errors.push(msg));
  assert.equal(filterStdin("ls", "original"), "original");
  assert.equal(filterStdin("ls", "again"), "again");
  assert.equal(errors.length, 1, "the hint is said once per process");
  assert.match(errors[0], /ketch install listepo\/rtok/);
});
