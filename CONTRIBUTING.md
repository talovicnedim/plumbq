# Contributing

Thanks for helping! Bug reports, ideas and pull requests are all welcome.

## Getting started

```bash
pnpm install
pnpm dev     # runs the app (Vite on port 5174)
pnpm check   # must pass before a PR is merged
```

`examples/jsonplaceholder` is a ready-made workspace to try things against.

## How the code is organized

```
crates/core/            All workspace, .http, environment and HTTP logic (Rust).
                        One module per concept: workspace, requests, folders, environments,
                        variables, send, history, curl, import.
crates/mcp/             MCP server (plumbq-mcp), thin tools over core: hosted in the app
                        over HTTP; the binary is a stdio bridge to it (--url) or a
                        standalone server for one folder (--workspace).
crates/assistant/       Built-in chat: providers and an agent loop over the MCP tools.
crates/cli/             Command-line runner (plumbq-cli).
apps/desktop/src-tauri/ Tauri shell: thin commands that call core, plus the file watcher.
apps/desktop/src/       SvelteKit UI (Svelte 5, TypeScript, Tailwind, shadcn-svelte).
scripts/                build-sidecars.mjs: builds mcp + cli for bundling.
```

## Building installers

`pnpm build` runs `tauri build` with `src-tauri/tauri.release.conf.json`, which adds the MCP
server and CLI as Tauri sidecars (`bundle.externalBin`) and builds them first with
`scripts/build-sidecars.mjs`. The base `tauri.conf.json` has no sidecars, so `pnpm dev` and
`pnpm check` never need them. Built sidecars go to `apps/desktop/src-tauri/binaries/`
(gitignored) as `<name>-<target-triple>`; `--target universal-apple-darwin` builds both Mac
architectures and merges them with `lipo`.

Release builds can also merge `src-tauri/tauri.updater.conf.json` (auto-updates), and CI signs
them when secrets are present. The steps are in [docs/RELEASING.md](docs/RELEASING.md).

The dependency direction is one-way: the app calls core, core knows nothing about the UI.

## Guidelines

- **Simple over clever.** Plain functions and data; no abstraction until there's a third use.
- **Files are the source of truth.** Core reads and writes the workspace on every call; the
  app follows changes through file watching.
- **Consistent naming across layers:** `requests::move_to` in core is the `requests_move`
  command and `api.requests.move()` in the UI.
- **Logic lives in core.** Commands and components stay thin.
- **Safety first.** Secrets stay in the keychain and are masked in anything returned; writes to
  production environments need confirmation, enforced in core.
- **Tests:** unit tests next to the code, using temporary workspaces and a local echo server.
  No network access in tests.
- **Commits:** [Conventional Commits](https://www.conventionalcommits.org) (`feat: …`,
  `fix(core): …`).
- TypeScript types for the backend are generated from Rust (`src/lib/api/bindings.ts`); don't
  edit that file by hand, run `cargo test` to regenerate it.
