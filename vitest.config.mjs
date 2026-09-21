// T119: vitest runs the TS host plugin tests. It is a mise tool, not a package dependency, so
// this file imports nothing and the tests use the injected globals (`test`, `expect`, `vi`).
export default {
    // Not node_modules/.vite: there is no package here, and target/ is already ignored.
    cacheDir: "target/vitest",
    test: {
        globals: true,
        include: ["plugins/**/*.test.ts"],
    },
};
