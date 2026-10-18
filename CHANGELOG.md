# Changelog

## 0.1.0 (2026-10-17)

First public release.

- Workspaces are folders of plain `.http` files (JetBrains HTTP Client and VS Code REST Client
  format), read and written losslessly: comments, directives, file variables, CRLF line endings
  and line order survive every save; files changed on disk reload live, and stale saves are
  refused instead of overwriting someone else's edit
- New workspace from scratch or a demo, workspace switcher (⌘P), recent workspaces, rename
  (a display name in `folder.toml`; the folder stays where it is)
- Folders with order and shared headers in `folder.toml`, drag and drop, duplicate, rename,
  workspace-wide variables and HTTP settings (proxy, custom CA, client certificates incl. PFX,
  timeouts, redirects, TLS verification)
- Request editor: URL, params, headers (with bulk edit) and body (JSON, form, text, XML, file,
  multipart) with highlighted `{{variables}}` that show their value and source on hover, auth
  (Bearer, Basic, API key, OAuth 2.0), docs, tags, assertions and per-request settings; custom
  HTTP methods
- Paste a cURL command into the URL bar to fill a request (its credentials are kept as secrets
  of the selected environment); copy any request as cURL, fetch, Python, PHP (Laravel Http), Go
  or HTTPie
- Environments in TOML with production / staging / development / local tags, secrets in the OS
  keychain, a production banner, and typed confirmation before writes to production
- Variables: environment, workspace, file (`@name = value`), dynamic (`$uuid`, `$timestamp`,
  `$randomInt`), `$prompt`, `$processEnv`, `$dotenv`, OAuth tokens (`$auth.token`), and request
  chaining (`{{login.response.body.$.token}}`)
- OAuth 2.0: authorization code with PKCE (browser sign-in) and client credentials, tokens
  cached in the keychain and refreshed automatically
- Responses: status, timing and size, pretty / raw / preview (images, PDF, HTML, audio,
  video), JSONPath filter, save to file, cancel, cookies, assertion results
- History on this machine, saved response examples committed next to the request, and compare
  any response with an example or an earlier send (side-by-side diff plus changed JSON fields)
- Cookie jar per workspace and environment
- Runner: run a folder or the whole workspace with live results, tags, delays, iterations and
  CSV / JSON data rows; JUnit and JSON reports
- `plumbq` CLI for CI (`run`, `docs`), with secrets from environment variables or `--var`
- Import Postman, OpenAPI 3, Swagger 2, Insomnia, HAR, Bruno and cURL; re-importing a spec
  updates it without duplicates and keeps your edits; credentials found in imports become
  `{{secret}}` placeholders instead of being written to files
- Export to Postman and to a self-contained HTML documentation page
- MCP server so AI assistants (Claude, ChatGPT, Cursor, VS Code) can organize, write, preview
  and send requests: secrets never reach the agent, and changes and risky sends need approval
- Built-in AI assistant (⌘J) with your own Anthropic, OpenAI, Gemini or OpenRouter key, or a
  local model (Ollama, LM Studio): same tools, permissions and approvals as MCP agents, tool
  calls shown as they run, chats saved per workspace, quick actions, and an option to keep
  response bodies from the model
- Secrets are masked everywhere they could appear (responses, history, errors, exports, agent
  output), including encoded forms
- ⌘K command palette, tabs with drag to reorder (they shrink to fit, with a list of all open
  tabs when they don't), keyboard-first editing, light and dark themes
- Signed auto-updates; macOS, Windows and Linux builds
