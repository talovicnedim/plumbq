# plumbq-mcp

MCP server for a Plumbq workspace. It lets AI assistants (Claude Code, Claude Desktop,
ChatGPT, Cursor, VS Code / Copilot, Codex) read, organize, write and send the requests in a
folder of `.http` files, and asks you in the app before anything changes.

It works the same way as the other apps in this family (Tugdeck, Rowcove): the server runs
**inside the desktop app** over HTTP, and every change an agent wants is **approved in a dialog**.

## Three ways to run it

**1. Inside the app (recommended).** The AI dialog (the AI button, ⌘⇧I) → AI apps
→ turn the server on. It serves Streamable HTTP on `http://127.0.0.1:7459/mcp/<token>` and works
on the workspace open in the app. The random token in the path is the only credential, so the
same URL works for every client; regenerate it any time. The dialog adds the server to Claude
Code, Claude Desktop, Cursor, VS Code or Codex with one click (`apps.rs` edits their config files
and backs them up once), and has copy-paste setup for each client:

| Client         | Setup                                                                                                                             |
| -------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| Claude Code    | `claude mcp add --scope user --transport http plumbq <url>`                                                                       |
| Claude Desktop | `{"mcpServers": {"plumbq": {"command": "<app>/plumbq-mcp", "args": ["--url", "<url>"]}}}`                                         |
| Cursor         | `{"mcpServers": {"plumbq": {"url": "<url>"}}}` in `~/.cursor/mcp.json`                                                            |
| VS Code        | `{"servers": {"plumbq": {"type": "http", "url": "<url>"}}}`                                                                       |
| Codex          | `[mcp_servers.plumbq]` with `command = "<app>/plumbq-mcp"`, `args = ["--url", "<url>"]`                                           |
| ChatGPT        | needs public HTTPS: `cloudflared tunnel --url http://127.0.0.1:7459`, then add the tunnel address + `/mcp/<token>` as a connector |

**2. As a bridge** (`plumbq-mcp --url <url>`) for AI apps that only start local commands
(Claude Desktop, Codex). The binary is bundled with the app; it forwards stdio to the server in
the app, so it needs no Node.js, works on the workspace open there, and approvals pop up in the
app as usual (`bridge.rs`).

**3. Standalone** (stdio) for one workspace folder, for headless use (e.g. a CI box) where the
app's server isn't running:

```bash
cargo build -p plumbq-mcp
plumbq-mcp --workspace ~/projects/my-api
plumbq-mcp --workspace ~/projects/my-api --read-only
plumbq-mcp --workspace ~/projects/my-api --allow-env --import-dir ~/Downloads
```

| Flag               | What it does                                                                   |
| ------------------ | ------------------------------------------------------------------------------ |
| `--read-only`      | Only read-only tools, plus sends limited to GET, HEAD and OPTIONS; never asks. |
| `--allow-env`      | Resolve `{{$processEnv NAME}}` / `{{$env.NAME}}` (otherwise the app setting).  |
| `--import-dir DIR` | Besides the workspace, the one folder `import_file` may read from.             |

It follows the permissions saved in the app and asks for approval through the running app
(request files in the app data dir). When the app isn't open, nobody answers and the change is
denied after 120 s (`PLUMBQ_APPROVAL_TIMEOUT_SECS` changes that). Either way, changes show up
in the app live through its file watcher, and sends appear in its history.

## Permissions and approvals

Every tool call gets a permission decided in code (`plumbq_core::permissions`), never by the
agent. AI → Permissions sets each to Allow, Ask or Deny. Its **Read-only** switch overrides
them (only reads and safe sends), and environments unticked there are hidden: tools don't list them
and any call that uses one is refused:

| Permission                     | Default | Tools                                                                                |
| ------------------------------ | ------- | ------------------------------------------------------------------------------------ |
| Read the workspace             | always  | tree, get, search, preview, history, examples, environments (no secret values)       |
| Send GET / HEAD / OPTIONS      | Allow   | `requests_send`, `folders_run`                                                       |
| Send requests that change data | Ask     | `requests_send`, `folders_run`                                                       |
| Create and edit requests       | Ask     | create, update, move, folders, reorder, headers, imports, examples                   |
| Change environments            | Ask     | `environments_create`, `environments_set_variable`                                   |
| Delete                         | Ask     | `requests_remove`, `folders_remove`, `environments_remove_variable`, `cookies_clear` |

Some cases always ask, whatever the setting:

- **Writes to production** (the chosen environment, or the host of any production environment)
  and changes to a production environment's variables need the strong confirmation: you type
  the environment's name.
- **Deleting** needs the strong confirmation (environment or workspace name).
- **Secrets to a new host:** a request using a keychain secret, `$dotenv`, `$processEnv`, an
  OAuth token, a chained response or a body file (`< ./data.json`), going to a host that isn't
  a URL of the environment.
- **Repointing an environment:** moving a URL variable to another host, or pointing a
  non-production environment at a production host (strong).
- **Downloads from unexpected hosts:** `import_url` from this machine, the local network, a
  production host or a host no environment points at.

The approval dialog shows who asks, the action, the environment, the agent's `reason`, and
exactly what would happen: the request as it would be sent (secrets as `{{name}}`) or a diff of
the file change. **Allow once**, **Always allow** (switches that kind of call to Allow; not
offered for production writes, deletes or risky sends) or **Deny**; denials tell the agent not
to retry. Every call is
recorded in the **AI activity log** (`activity.rs`, AI → Activity, shared with the built-in
chat; the last 1000 calls): who, what, the environment, and whether it ran, was approved,
denied (by you, or nobody answered), blocked (by the settings) or failed, with credentials
redacted.

## Safety

- **Secrets never reach the agent.** Secret, `$prompt`, `.env` and OAuth values are masked as
  `{{name}}` in everything returned; agents can't read or set secrets. Imports turn literal
  credentials (HAR `Authorization`, `curl -u`, Postman bearer tokens, secret-looking environment
  variables) into secret names without values; the user sets them in the app.
- **Imports read only the workspace** (and `--import-dir`), so an agent can't read or probe
  other files. The approval lists every environment an import creates or adds to, with its
  variables; agents never add to a production or hidden environment. `import_url` downloads
  the spec under the "Send GET" permission first, so the approval can show what it imports;
  every redirect is checked like the first URL. `import_file` never reads `environments/` or
  `.env` files, and parse errors never quote the file.
- **Redirects stay where they should:** an agent's send doesn't follow a redirect to another
  origin on this machine, the local network or a production host, also when a public-looking
  name resolves there. A host the request reaches may lead there only when an environment points
  at it (dev domains like `api.myapp.test`), whether or not the user approved the send. The
  request's own host may also when it ends in `.test`, `.example` or `.invalid`.
- **The dialog shows what is really there:** names, values, paths and the agent's reason are
  shown with line breaks and Unicode format characters escaped, so they can't fake lines.
- **Never sent by an agent:** files in `environments/`, `.env` files, and body files whose name
  comes from a secret or a response.
- **What you approved is what is sent:** a request that changes while you are asked, or during
  an approved `folders_run`, is refused instead of sent. Cancelling a call withdraws its
  approval and stops a run.
- **Read-only mode** (stdio `--read-only`) offers only tools annotated read-only plus
  `requests_send` and `folders_run`, which then send only GET, HEAD and OPTIONS. It sends cookies
  from the app's jar but never writes to it; so does the app's Read-only switch.
- **Cookie values stay hidden:** responses show `Set-Cookie` names only, and `cookies_list`
  never returns values. A server that echoes request headers back can still show them.
- **No accidental deletes of work:** `folders_remove` only removes folders holding nothing but
  `folder.toml` files and empty folders (saved examples count as content); never the workspace
  root, hidden folders or `environments/`.
- **No writes to the wrong request:** `requests_get` returns a `version`. Passing it to
  `requests_update`, `requests_move` or `requests_remove` refuses the change if the file changed
  since; the agent reads it again. Updates and deletes also fail when the request changed while
  you were asked.
- **History follows reorganizing** so `history_compare` and chaining keep working.
- **One change at a time:** tools that change files take turns, also between the chat and AI
  apps.
- **Approvals through files** (stdio server) trust the local user account: any process of the
  same user could write an answer file, just as it could edit the workspace or read the
  keychain. MCP tools themselves can't: they only touch paths inside the workspace.

## Prompts

`organize_workspace`, `write_assertions`, `document_api` and `debug_failing_request` are
ready-made tasks clients show in their prompt menu (e.g. `/` in Claude).

## Tools

| Tool                           | What it does                                                     |
| ------------------------------ | ---------------------------------------------------------------- |
| `workspace_tree`               | Outline of folders and requests; `folder` / `depth` to narrow.   |
| `requests_search`              | Find requests by words, tag and method.                          |
| `requests_get`                 | A request as `.http` text, with a `version` for safe writes.     |
| `requests_preview`             | What a request would send, without sending; may it be sent.      |
| `requests_create`              | New request from `.http` text, in its own file.                  |
| `requests_update`              | Replace a request with new `.http` text.                         |
| `requests_move`                | Move to another folder and/or position (group, reorder).         |
| `requests_move_many`           | Many moves in one call, in order; stops at the first failure.    |
| `requests_remove`              | Delete a request.                                                |
| `requests_send`                | Send and return status, timing, useful headers, capped body.     |
| `folders_create`               | Create a folder.                                                 |
| `folders_rename`               | Rename a folder.                                                 |
| `folders_duplicate`            | Copy a folder with its requests, headers and examples.           |
| `folders_remove`               | Remove an empty folder.                                          |
| `folders_reorder`              | Set the order of a folder's children.                            |
| `folders_get`                  | A folder's shared headers and order; root defaults and settings. |
| `folders_set_headers`          | Add, change or remove shared headers (merges by default).        |
| `environments_list`            | Environments, tags, variables and secret names.                  |
| `environments_create`          | Create an environment.                                           |
| `environments_set_variable`    | Set a plain variable (not secrets).                              |
| `environments_remove_variable` | Remove a plain variable.                                         |
| `history_list`                 | Past sends of a request.                                         |
| `history_get`                  | One past response.                                               |
| `history_compare`              | What changed between two sends (status, timing, JSON fields).    |
| `examples_list`                | Saved example responses of a request.                            |
| `examples_create`              | Keep a past response as a named example next to the request.     |
| `folders_run`                  | Run a folder like a test suite: one line per request + summary.  |
| `import_curl`                  | Create a request from a cURL command.                            |
| `import_file`                  | Import Postman, OpenAPI, Swagger, Insomnia, HAR or Bruno.        |
| `import_url`                   | Import a spec from a URL; re-importing updates, no duplicates.   |
| `cookies_list`                 | Stored cookies of an environment (names and domains, no values). |
| `cookies_clear`                | Delete an environment's cookies, e.g. to start a fresh session.  |

Every tool declares whether it's read-only. Read-only mode keeps only those (plus the two send
tools), so a new write tool can't slip in by accident.
