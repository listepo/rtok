// T310.2: web/src/api/snapshot.gen.ts from ws.schema.json, which Rust generates from its /ws
// types (src/web/protocol.rs). `--check` fails instead of writing when the committed TS is stale.
import { readFile, writeFile } from "node:fs/promises";
import { compile } from "json-schema-to-typescript";

const dir = new URL("../src/api/", import.meta.url);
const schemaUrl = new URL("ws.schema.json", dir);
const outUrl = new URL("snapshot.gen.ts", dir);

const schema = JSON.parse(await readFile(schemaUrl, "utf8"));
const ts = await compile(schema, "WsProtocol", {
    bannerComment:
        "// Generated from ws.schema.json by scripts/gen-api.mjs (T310.2). Do not edit: change the Rust\n// types in src/web, bless the schema, then run `npm run gen:api`.",
    additionalProperties: false,
    unreachableDefinitions: true,
});

if (process.argv.includes("--check")) {
    const have = await readFile(outUrl, "utf8").catch(() => "");
    if (have !== ts) {
        console.error("src/api/snapshot.gen.ts is stale: run `npm --prefix web run gen:api`");
        process.exit(1);
    }
} else {
    await writeFile(outUrl, ts);
}
