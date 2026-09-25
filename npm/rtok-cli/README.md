# rtok-cli

One Rust binary that cuts tokens for coding agents: hooks, an MCP server and an API proxy.
This npm package installs the prebuilt native binary; Node only resolves it.

```bash
npm i -g rtok-cli      # or: npx rtok-cli --help
rtok --version
```

The binary comes from a platform package (`rtok-cli-darwin-arm64`, `rtok-cli-linux-x64-gnu`,
`rtok-cli-win32-x64-msvc`) that npm picks through `optionalDependencies`, the esbuild layout. On
macOS and Linux the install step puts the native binary itself on `PATH`; on Windows, or with
`--ignore-scripts`, a small Node launcher runs it. Do not install with `--omit=optional`.

Docs, other install paths and the license terms: https://github.com/listepo/rtok
