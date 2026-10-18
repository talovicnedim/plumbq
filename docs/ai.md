# AI in Plumbq

Plumbq, Tugdeck and Rowcove share one AI design. Each is a standalone repo with its own
code, but the screens, wording, names and rules are the same.

## Two ways in, one set of rules

- **Chat** is the built-in assistant (⌘J). You bring your own key, or use a local model.
- **AI apps** are the MCP clients (Claude Code, Claude Desktop, Cursor, VS Code, Codex,
  ChatGPT) connected to the app's MCP server.

Both go through the **same tools**. The chat connects to the app's own MCP server in-process
(`crates/assistant/src/tools.rs`), so it can't skip anything an outside app has to go through:

- **The same permission decision.** Code decides (`core::permissions::decide`), never the model.
- **The same approval dialog.** It shows who asks, the action, the AI's reason, the environment
  with its badge, and the exact request or diff; production writes and deletes need you to type
  the environment's name (checked in Rust, `ApprovalRequest::confirm_name`); it denies on its
  own after 120 s and brings the window forward. **Always allow** also switches that kind of
  call to Allow, for the chat and every AI app in every workspace; it is offered only where a
  rule can cover the call (`always_allow`), never for production writes, deletes or risky sends.
- **The same activity log** (`crates/mcp/src/activity.rs` → `ai-activity.jsonl`, redacted, the
  last 1000 calls).

## Where things live

| Concept         | Rust                                                        | Tauri commands                                                                                                                                                                                                    | Frontend                                                                                                                                   |
| --------------- | ----------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| Built-in chat   | `crates/assistant`                                          | `assistant_status`, `assistant_update`, `assistant_provider_new`, `assistant_key_set`, `assistant_models`, `assistant_test`, `assistant_quick_action(s)`, `assistant_send`, `assistant_stop`, `assistant_chats_*` | `state/assistant.svelte.ts`, `ai/ChatPanel.svelte`, `ai/ChatAnswer.svelte`, `ai/ChatHistory.svelte`, `ai/ToolCard.svelte`, `utils/chat.ts` |
| Providers       | `assistant/{anthropic,openai,gemini}.rs`, `provider.rs`     | -                                                                                                                                                                                                                 | `utils/providers.ts`, `ai/ProviderLogo.svelte`                                                                                             |
| MCP server      | `crates/mcp` (`server/`, `tools/`, `bridge.rs`)             | `mcp_status`, `mcp_update`, `mcp_token_regenerate`, `mcp_binary`                                                                                                                                                  | `state/ai.svelte.ts`, `ai/AiRoot.svelte`, `ai/McpStatus.svelte`                                                                            |
| One-click setup | `crates/mcp/src/apps.rs`, `setup.rs` (snippets)             | `mcp_apps_list`, `mcp_apps_connect`, `mcp_apps_disconnect`, `mcp_clients_list`                                                                                                                                    | `ai/AppsTab.svelte`, `ai/ClientTabs.svelte`, `ai/OneClickRow.svelte`, `ai/ServerSettings.svelte`                                           |
| Approvals       | `crates/mcp/src/approval.rs`, `core/src/approvals.rs`       | `mcp_approvals_list`, `mcp_approvals_answer`; event `mcp-approval`                                                                                                                                                | `ai/ApprovalDialog.svelte`, `ai/ApprovalForm.svelte`, `utils/approvals.ts`                                                                 |
| Activity log    | `crates/mcp/src/activity.rs`                                | `mcp_activity_list`, `mcp_activity_clear`                                                                                                                                                                         | `ai/ActivityTab.svelte`                                                                                                                    |
| Keychain        | `core/src/app_secrets.rs` (on top of `core/src/secrets.rs`) | -                                                                                                                                                                                                                 | -                                                                                                                                          |
| Explain this    | -                                                           | -                                                                                                                                                                                                                 | `assistant.explain()` in `state/assistant.svelte.ts`, `ai/AskAi.svelte`, `ASK` in `utils/ai.ts`                                            |
| Dialog          | -                                                           | -                                                                                                                                                                                                                 | `ai/AiDialog.svelte` with `ChatTab`, `AppsTab`, `PermissionsTab`, `ActivityTab`                                                            |

Plumbq keeps provider keys in `core/src/app_secrets.rs`: `core/src/secrets.rs` is already
the keychain store for workspace secrets, and app secrets use it under a scope no workspace can
produce.

## The AI dialog

Open it with the **AI** button in the header, ⌘⇧I, the command palette, the settings button in
the chat panel, or Settings → AI. The top right has **Turn AI off**, which turns off the chat,
the MCP server and every "Explain" button until you turn AI back on. **Turn AI on** brings the
chat back, and the MCP server too if it is enabled. Settings and keys are kept.

| Tab         | What's in it                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Chat        | Six provider tiles (Ollama, LM Studio, Anthropic, OpenAI, Gemini, Custom). Picking a cloud one asks once what gets sent to it. Then the API key (kept in the keychain, with a "Get a key" link), Model + Refresh + Test, and Advanced (base URL, temperature).                                                                                                                                                                                                                                               |
| AI apps     | Client tabs, each with a one-click **Connect** button and the setup to copy as a fallback. Connect edits the app's config file (`apps.rs`, through a symlink to its real place) and backs it up before each edit. Claude Code, Cursor and VS Code use the address; Claude Desktop and Codex start the bundled `plumbq-mcp --url <address>` bridge. ChatGPT uses a tunnel instead. Server settings holds the port, regenerating the address, turning the server off, and running it headless without the app. |
| Permissions | The **Read-only** switch, the fixed rules as bullets, and **Environments the AI can use** (unticked = hidden). Then: share response bodies, Allow / Ask / Deny per kind of action, and OS environment variables.                                                                                                                                                                                                                                                                                             |
| Activity    | Every call from both ways in, newest first, with a **Source** badge ("Chat" or the AI app's name), the environment, the outcome (`ran`, `approved`, `denied`, `blocked`, `failed`) and a short redacted summary. Clear empties it.                                                                                                                                                                                                                                                                           |

Custom is any OpenAI-compatible URL; it defaults to OpenRouter. Settings saved before this, which
had a separate OpenRouter provider, load as Custom with their URL and key kept. Activity logs
written before the outcome rename (`allowed`) load as `ran`.

## The `plumbq-mcp` binary

- `plumbq-mcp --url <address>`: a stdio bridge to the server in the app (`bridge.rs`). Claude
  Desktop and Codex start it. It connects as the AI app that started it, so approvals and the
  activity log show that app's name, and it reconnects after the app restarts.
- `plumbq-mcp --workspace <folder>`: the same tools for one folder without the app's server,
  for headless use only. Approvals go to the running app through request files, and are denied
  when nobody answers.

The app finds the binary next to its own executable (a Tauri sidecar in bundles,
`target/debug` in development), like Rowcove.

## Rules (enforced in Rust)

- **Read-only** (`McpSettings::read_only`) makes the permissions `AgentPermissions::read_only()`.
  That means reads and GET / HEAD / OPTIONS sends only, for the chat and AI apps alike.
- **Hidden environments** (`McpSettings::hidden_environments`, matched by name in any letter
  case in every workspace) are left out of `environments_list`. Any send, run, cookie or environment call that
  names one is refused and logged as `blocked`.
- **Turned off** (`McpSettings::turned_off`, in `mcp.json` so the stdio server sees it too)
  refuses chat turns and stops running ones, stops the server in the app, denies waiting
  approvals, and makes every MCP call fail, also on the stdio server. `enabled` stays as it was.
- **Cloud consent**: the chat refuses to send to a cloud provider until the user accepted its
  "what gets sent" notice (`AssistantSettings::chat_provider`).
- Every tool that changes something takes an optional `reason`, shown in the approval dialog.
- Secrets never reach the AI; they show as `{{name}}`.
- One-click Connect writes AI app configs, which hold the secret server URL, atomically and
  readable by the user only (`0600`, folders it creates `0700`); existing files keep their
  mode, backups are always `0600`.

## Chat panel

- **Header:** "Provider · model", recent chats, New chat, Settings (opens AI → Chat), Close.
- **Not set up:** "Pick a model to chat" and "Or use Claude Code, Cursor, VS Code, Codex or
  ChatGPT".
- **While it runs:** tool calls show as cards, Stop ends a turn, and a turn stops after 12
  steps.
- **Quick actions** come from the MCP prompts. The "This request", "This folder" and "Last
  response" chips attach context.

## "Explain this"

`assistant.explain(prompt, attach)` in `lib/state/assistant.svelte.ts` opens the chat and asks. If no model
is set up yet, it keeps the question and sends it once one is. The button is `AskAi.svelte`, and
the prompts are `ASK` in `lib/utils/ai.ts`.

| Where                        | Button                                                |
| ---------------------------- | ----------------------------------------------------- |
| Send failed (response panel) | Why did this fail?                                    |
| Response header              | Why did this fail? (4xx/5xx) or Explain this response |
| Tests tab, failed assertion  | Explain                                               |
| Request → Assertions tab     | Write tests for this                                  |
| Request → Docs tab           | Document this                                         |

## Tools

The MCP tools, their permissions and safety rules are listed in
[`crates/mcp/README.md`](../crates/mcp/README.md).
