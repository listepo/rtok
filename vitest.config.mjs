// T111: vitest runs the TS host plugin tests. It is a mise tool, not a package dependency, so
// this file imports nothing and the tests use the injected globals (`test`, `expect`, `vi`).

// Per-test limit. Every case spawns a fake `rtok` (a fresh `node` process, tests/node/fake-rtok.ts)
// one or more times, and `cargo nextest` runs these files (tests/pi_plugin.rs, tests/filter.rs)
// beside one other test per CPU. On a loaded box a `node` cold start alone can take seconds, so
// vitest's 5 s default failed cases that finish in well under a second when run alone. This is
// a hang guard, not a latency gate: the cases that assert timing (the wedged/aborted `rtok`
// ones) set their own limits and elapsed-time bounds.
const TEST_TIMEOUT_MS = 30_000;

export default {
    // Not node_modules/.vite: there is no package here, and target/ is already ignored.
    cacheDir: "target/vitest",
    test: {
        globals: true,
        include: ["plugins/**/*.test.ts"],
        testTimeout: TEST_TIMEOUT_MS,
    },
};
