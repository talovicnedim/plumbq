# Security policy

The app stores API credentials and sends requests to production systems, so security reports are
taken seriously.

## Reporting a vulnerability

Please **don't open a public issue**. Instead, use
[GitHub's private vulnerability reporting](https://github.com/talovicnedim/plumbq/security/advisories/new)
for this repository.

Include what you found, how to reproduce it, and the impact you expect. You'll get an answer
within a few days, and credit in the release notes if you'd like.

## Supported versions

Only the latest release receives security fixes.

## Design notes

- Secret values live in the OS keychain (or `PLUMBQ_SECRET_<NAME>` variables in CI) and are
  masked as `{{name}}` in responses, history, exports and agent output. Masking hides values; it
  doesn't decide where they're sent.
- AI agents (the MCP server) work under permissions decided in code (`core::permissions`):
  reading is allowed, sending safe requests is allowed by default, and everything that changes
  data or files asks the user in the app first, showing the exact request or a diff. Some cases
  always ask, whatever the settings: writes to a production environment or host (typing its
  name), deletes, secrets going to a host that isn't a URL of the environment, and moving an
  environment URL to another host (`core::guard` detects these). The process environment is off
  unless allowed. Every call is written to a redacted activity log in the app data dir. An agent
  can still read anything the workspace files contain, so don't put secret values in them;
  imports move literal credentials out of the files for this reason.
- The built-in assistant sends your questions, its answers and what its tools return to the
  provider you choose (nothing leaves the machine with a local model). Tool output is the same
  masked text MCP agents get, so secret values never reach a provider; response bodies can be
  left out (AI → Permissions). Provider API keys live in the OS keychain and are removed
  from provider error messages. Its tool calls go through the same permissions and approvals
  as MCP agents and appear in the activity log as `Chat · <model>`.
- The in-app MCP server listens on `127.0.0.1` only; the random token in its URL path is the
  credential (settings file mode 0600). The `plumbq-mcp --url` bridge (Claude Desktop, Codex)
  only forwards to that server. The standalone stdio server asks for approvals through request
  files in the app data dir that the running app answers; another process of the same user
  could write an answer, the same trust boundary as the keychain and the workspace itself.
- Workspace files are written atomically, and writes to an existing request are refused with a
  conflict if the file changed since it was read, so a stale editor or agent can't overwrite or
  delete a different request. Folders are only deleted without confirmation when they hold
  nothing but organizing files.

## Scope

Especially interesting:

- Secret values ending up in workspace files, history, logs, "copy as cURL", or the UI
- Ways to send a data-changing request to a production environment without confirmation
- Ways for an agent connected over MCP to send a secret to a host the user didn't configure, to
  change files or send writes without the user's approval when the settings say Ask, to approve
  its own requests, or to change files in `--read-only` mode
- Ways to reach the in-app MCP server without its token
- Paths or ids that read or write files outside the opened workspace
- Imported files (Postman, OpenAPI, Insomnia, cURL) that can write outside the workspace or run
  code
- Anything that lets a response or workspace file run code in the app
