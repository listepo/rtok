// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

// Writes `.br` and `.gz` beside every text asset in dist/ so `rtok web` can serve the
// encoding the browser asks for without compressing on each request (T310.9). Node's
// zlib is enough: no dependency, and brotli at quality 11 is a one-off build cost.
import { brotliCompressSync, constants, gzipSync } from "node:zlib";
import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { extname, join } from "node:path";

const root = new URL("../dist", import.meta.url).pathname;
// Already-compressed formats (woff2, png, ...) gain nothing and would double the binary.
const TEXT = new Set([".html", ".js", ".css", ".svg", ".json", ".txt", ".map"]);
const MIN_BYTES = 256;

function* walk(dir) {
    for (const name of readdirSync(dir)) {
        const path = join(dir, name);
        if (statSync(path).isDirectory()) yield* walk(path);
        else yield path;
    }
}

let count = 0;
for (const path of walk(root)) {
    if (!TEXT.has(extname(path))) continue;
    const bytes = readFileSync(path);
    if (bytes.length < MIN_BYTES) continue;
    writeFileSync(
        `${path}.br`,
        brotliCompressSync(bytes, {
            params: {
                [constants.BROTLI_PARAM_QUALITY]: 11,
                [constants.BROTLI_PARAM_SIZE_HINT]: bytes.length,
            },
        }),
    );
    writeFileSync(`${path}.gz`, gzipSync(bytes, { level: 9 }));
    count += 1;
}
console.log(`precompress: ${count} files`);
