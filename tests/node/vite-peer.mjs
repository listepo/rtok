// T83.4: on Windows, mise's npm installer leaves `@vitest/mocker` without its `vite` peer link,
// and Node's ESM resolver ignores NODE_PATH. `tests/common/mod.rs` preloads this file with
// `--import` and names the `npm:vite` install in RTOK_VITE_ROOT; a bare `vite` import that
// does not resolve where it stands is retried from there. Elsewhere the first try succeeds.
import { registerHooks } from "node:module";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const root = process.env.RTOK_VITE_ROOT;
if (root) {
  const parentURL = pathToFileURL(join(root, "peer.mjs")).href;
  registerHooks({
    resolve(specifier, context, next) {
      try {
        return next(specifier, context);
      } catch (error) {
        if (specifier !== "vite" && !specifier.startsWith("vite/")) throw error;
        return next(specifier, { ...context, parentURL });
      }
    },
  });
}
