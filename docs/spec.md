# Plumbq - product spec

Started 2026-10-11. "The app" below means Plumbq (`plumbq-*` crates).

## Vision

A fast, free, local-first API client that your AI assistant can work in. Requests are plain
`.http` files in your repo. Connect Claude, ChatGPT, Cursor or any MCP client and ask it to
organize, write, run and debug your requests, with every change shown as a diff you approve.

## Why

- Postman and Insomnia push accounts and cloud sync; their AI features only work in their cloud
  with their model.
- Bruno and Yaak fixed local-first, but treat AI as an afterthought and use their own file
  formats.
- The app: plain `.http` files (already understood by VS Code and JetBrains), strict safety
  around production, and an MCP server as a first-class way to use it.

## Principles

1. **Free to use, local first.** MIT, no accounts, no telemetry. (Business model still open; no "free forever" promise.)
2. **Your files, your format.** A workspace is a folder you can commit. No export step.
3. **Bring your own AI.** We never pay for or proxy model calls; the user's assistant does the
   thinking.
4. **AI proposes, you approve.** Agent edits arrive as a changeset with a diff; applied changes
   can be undone.
5. **Safe around production.** Secrets never reach files or the AI; writes to production
   environments need confirmation.

## Non-goals (for now)

- Accounts, cloud sync, team workspaces.
- A JavaScript scripting sandbox (pre-request / test scripts). Variables, chaining and
  assertions cover most needs without running arbitrary code.
- Load testing, API design / mock-first workflows, documentation publishing.

## Workspace format

A workspace is any folder. Real directories are folders in the sidebar. There is no workspace
manifest: the root `folder.toml` plays the role of workspace settings. The workspace is called by
its folder name, unless the root `folder.toml` sets `name = "..."` ("Rename workspace" writes it;
the folder keeps its name, so keychain secrets and history stay with it). "New workspace" creates
a folder with a `local` environment and an example request.

```
my-api/
  folder.toml              root order and headers shared by every request
  environments/
    local.toml             variables, tag = "local" | "development" | "staging" | "production"
    production.toml
  users/
    folder.toml            order of children, shared headers / auth
    list-users.http
    create-user.http
  auth/
    login.http
```

- **One request per `.http` file**, so git diffs and agent edits stay small. Files containing
  several `###`-separated requests are read fine and shown as a group.
- **Order** lives in `folder.toml` (`order = ["list-users.http", ...]`); missing entries sort
  alphabetically after the listed ones. This is what makes "reorder" and "group" possible.
- **Secrets** are variables whose value lives in the OS keychain; the TOML stores only the name.
  Values never appear in files, history exports, logs or MCP output.
- Extensions to the `.http` format use `# @` directive comments other tools ignore, e.g.
  `# @name login`, `# @assert status == 200`.

## Roadmap

✅ done · ⏳ in progress · ☐ planned.

### v0.1 - a good API client

- ✅ Open a folder as a workspace, create a new one, switch between recent workspaces (⌘P)
- ✅ Sidebar tree with folders, order from `folder.toml`, drag-and-drop moves; file watching so
  external edits (git, editors, agents) show up live
- ✅ `.http` parser and writer, compatible with JetBrains / VS Code REST Client, keeping
  comments and response handlers
- ✅ Request editor: method, URL, params, headers, body with JSON highlighting, assertions;
  hovering a `{{variable}}` in the URL, header and param values or body shows its value and
  source, missing ones are underlined
- ✅ Request editor: form / multipart / file bodies, auth helpers (Bearer, Basic, API key)
- ✅ File variables (`@name = value`), custom methods, per-request directives (`@no-redirect`,
  `@timeout`, `@connection-timeout`, `@no-cookie-jar`, `@insecure`), workspace HTTP settings
  (`[settings]` in the root `folder.toml`: timeouts, redirects, TLS, proxy, CA / client certs
  incl. PKCS#12 with the password in the keychain)
- ✅ Environments with variables, colored tags, keychain secrets; folder-level shared headers;
  workspace-wide variable defaults in the root `folder.toml`
- ✅ Request docs (markdown comment lines) and `# @tag` tags (`tag:` filter, ⌘K, CLI `--tag`)
- ✅ Duplicate folder, bulk edit for headers/params, tab context menu and drag to reorder,
  natural sort
- ✅ Cookie jar per workspace and environment (in the app data dir), Cookies tab, MCP
  `cookies_list` / `cookies_clear`, CLI `--no-cookies`
- ✅ Variables including `$uuid`, `$timestamp`, `$isoTimestamp`, `$randomInt`
- ✅ `$prompt name` (asked at send time, masked), `$processEnv NAME` / `$env.NAME` (masked when the
  name reads as a credential, e.g. `API_TOKEN`, `DB_PASSWORD`, `GITHUB_PAT`), `$dotenv NAME` from the workspace `.env` (masked)
- ✅ Response viewer: pretty JSON, headers, status, timing, size, what was sent
- ✅ Per-request history (JSON files in the app data dir, last 50 per request)
- ✅ Compare a response with an earlier one or a saved example (side-by-side diff + changed JSON
  fields; MCP `history_compare`)
- ✅ Saved response examples next to the request (`<file>.examples.json`; MCP `examples_list`)
- ✅ Copy as cURL, fetch, Python, Laravel Http, Go, HTTPie
- ✅ Save response body to a file; previews for images, PDF, sandboxed HTML, audio and video
  (binary bodies kept as base64; history skips those over ~1 MB)
- ✅ JSON path filter on responses with `[*]` / `.*` wildcards (`json_path::query`)
- ✅ Cancel an in-flight request (button, Esc, ⌘.); cancelled sends never reach history
- ✅ cURL import (paste, incl. `-F` multipart and `--data-urlencode`; its credentials are kept
  as secrets of the selected environment) and copy as cURL (secrets left as `{{name}}`)
- ✅ Import Postman collections, OpenAPI 3 / Swagger 2 (file or URL), Insomnia exports, HAR
  recordings and Bruno collections (`.bru` or YAML). Re-importing merges via `# @source METHOD
url | Imported name` directives: adds new requests, applies source renames unless the user
  renamed it, keeps user edits, reports stale ones; requests sharing a key match by name
- ✅ Data safety: `.http` writes are atomic and lossless; writes to an existing request carry the
  source the client read and fail with `conflict` if the file changed (UI: reload or save as
  new; MCP: `version`); moves, renames and removals report id changes so tabs and history
  follow; environment files keep comments and unknown keys; secrets are removed only explicitly
- ✅ Tabs with unsaved-change tracking, ⌘↵ send, ⌘S save, ⌘W close; tabs shrink to fit and a
  menu lists them all when they don't
- ✅ ⌘K command palette (`>` commands, `#` tags, `@` environments), resizable editor / response
  split
- ✅ Production banner in the editor while a production environment is selected
- ✅ Safety: non-GET/HEAD/OPTIONS on production environments needs typing the environment name,
  enforced in core
- ✅ **MCP server**, inside the app (Streamable HTTP on `127.0.0.1:7459/mcp/<token>`, like
  Tugdeck), reached by Claude Desktop and Codex through the bundled `plumbq-mcp --url` stdio
  bridge, and standalone for one folder (`--workspace`, headless); one-click setup and snippets
  for Claude Code, Claude Desktop, Cursor, VS Code, Codex and ChatGPT (tunnel)
- ✅ Agent permissions (Allow / Ask / Deny per kind of action, `core::permissions`), approval
  dialog with the exact request or file diff (strong confirmation for production writes and
  deletes), approvals from the stdio server through request files (like Rowcove), redacted AI
  activity log; MCP prompts `organize_workspace`, `write_assertions`, `document_api`,
  `debug_failing_request`

### v0.2 - AI that does real work

- ☐ Changesets: agent batch edits shown as a reviewable diff in the app, apply / reject, undo
- ✅ Chaining: `{{login.response.body.$.token}}`
- ✅ Assertions (`# @assert status == 200`, `body.$.path`, `header … contains`) run on every
  send, in the app, the MCP server and the CLI
- ✅ CLI runner for CI (`plumbq run <folder> --env staging`, pretty / JUnit / JSON reporters)
- ✅ Shared runner (`runner/`) for app, CLI and MCP `folders_run`: tags, delay, iterations,
  CSV/JSON data files (rows as variables), live results in the app, JUnit/JSON export
- ✅ Export to Postman v2.1 (+ environments) and single-page HTML docs (`plumbq docs`)
- ✅ OAuth 2 (auth code + PKCE, client credentials)
- ✅ MCP over local HTTP (in the app); ChatGPT through a tunnel (`cloudflared`), token in the path
- ✅ Built-in chat panel (⌘J): bring your own key (Anthropic, OpenAI, Gemini, or any
  OpenAI-compatible URL such as OpenRouter) or a local model (Ollama, LM Studio); runs on the app's MCP tools in-process (`crates/assistant`),
  so permissions, approvals and the activity log are shared; chats per workspace; response
  bodies can be kept from the model
- ✅ One AI dialog (Chat / AI apps / Permissions / Activity), one-click setup in AI apps,
  Read-only switch, hidden environments, "Explain this" buttons, "Turn AI off" (see
  [ai.md](ai.md))

### Later

GraphQL with
schema autocomplete, WebSocket / SSE, gRPC, Laravel route discovery (`route:list --json` +
FormRequest rules), mock server.

## AI and MCP

The full design (shared with Tugdeck and Rowcove) is in [ai.md](ai.md).

The MCP server runs inside the app (Streamable HTTP, the workspace open in the app); Claude
Desktop and Codex reach it through the bundled `plumbq-mcp --url` bridge, and
`plumbq-mcp --workspace` serves one folder headless. All work on the files directly; the app
picks up changes through file watching. What an agent may do is decided in code per call
(`core::permissions`): reads run, safe sends run by default, everything else asks the user in an
approval dialog that shows the exact request or a diff. Every call goes to a redacted activity
log.

Tools follow the project naming rule (`<resource>_<verb>`); a few are named for the AI and
differ from the Tauri commands (listed in `.claude/rules/project.md`). The full list is in
[`crates/mcp/README.md`](../crates/mcp/README.md). Still planned: `changeset_propose`
(batch edits the user reviews before anything changes).

Example asks: "group these by resource and sort CRUD-style", "move hardcoded tokens into
environment variables", "add status assertions to every request", "run login then /orders on
staging and explain the 403".

## Architecture

```
  apps/desktop (Svelte ⇄ Tauri, thin)   crates/mcp (rmcp, thin)   crates/cli (thin)
                     └───────────────────────┼──────────────────────┘
                                   ┌─────────▼─────────┐
                                   │ crates/core       │ workspace files, .http parser,
                                   │                   │ environments, HTTP engine, history
                                   └───────────────────┘
```

Rust 2024, Tauri 2, SvelteKit (Svelte 5, SPA), Tailwind, shadcn-svelte, specta for TS types,
`reqwest` for HTTP, `keyring` for secrets, `notify` for file watching.

- **Per-machine data** lives in the OS app data dir, never in the workspace: recently opened
  workspaces (`recent.json`) and history (`history/<workspace>/<request>/<id>.json`, the last 50
  per request, secrets already masked).
- **Selected environment** is remembered per workspace in the UI's local storage.
- **Dev server** runs on port 5174 (5173 is left free for other Tauri projects).

## Distribution

GitHub Releases for macOS, Windows and Linux via `tauri-action`; MCP server and CLI as static
binaries. MIT. Funding, if any: GitHub Sponsors.

## Open questions

- How far chaining goes before it needs scripting. Assertion syntax is documented in
  [formats.md](formats.md); `matches` (regex) isn't supported yet.
- Windows from day one? (No SSH dependency, so probably yes; needs testing of keychain and
  file watching.)
