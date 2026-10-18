# Formats and rules in detail

The longer explanations behind core modules. Each section names the module it belongs to.

## `.http` files (`http_file`)

Requests are stored in the format of the JetBrains HTTP Client and VS Code REST Client, so a
workspace stays usable from those editors.

```http
### Create user
# @assert status == 201
POST {{baseUrl}}/users
Content-Type: application/json

{"name": "Ada"}
```

Also understood: REST Client file variables (`@host = api.test`, usable as `{{host}}` in every
request of the file), `# @name login` reference names, the settings directives below and any
method token (`PURGE`, `PROPFIND`, …).

`Authorization: Basic user:password` and `Authorization: Basic user password` are base64-encoded
when the request is sent, so the credentials can use `{{variables}}`. "Copy as code" uses each
client's own basic auth option (`curl -u`), so secrets stay `{{name}}` there.

Saving is lossless. Every request remembers the exact text it was read from. An unchanged
request is written back byte for byte. An edited one is rewritten line by line against that
text, so comments, response handlers, line endings, a BOM and `HTTP/1.1` survive and only
edited lines change.

One problem comes from the format itself: a body line that starts with `###` starts a new
request when the file is read again. `body_warnings` flags it before saving. Any other write
that would read back differently (two requests joined into one, a lowercase method read as a
URL) is refused.

Ids of later requests in a multi-request file go by position (`all.http#3`). Removing or moving
a request shifts them, so those operations return the id changes, and open tabs and history
follow. Because ids use `#`, a file or folder with `#` in its name isn't shown. Writes to an existing request carry the text the caller read. If the file changed on
disk in between, they fail with `conflict` instead of changing the wrong request.

## Request settings (`request_settings`)

Per-request HTTP settings are JetBrains HTTP Client directives in the `.http` file, so they stay
with the request and keep working in JetBrains IDEs:

```http
# @no-redirect
# @no-cookie-jar
# @timeout 60
# @connection-timeout 5
# @insecure
GET https://self-signed.local/health
```

`@insecure` (skip TLS certificate checks) is ours; JetBrains ignores it. Durations are seconds,
or take a unit: `500 ms`, `30 s`, `2 m`.

## Workspace name (`workspace::name`)

`name = "Jellyreach API"` at the top of the root `folder.toml` is the name the app shows (sidebar,
switcher, recent lists, ⌘K, approvals). Without it, or when it is empty, the folder's name shows.
It is one line of up to 80 characters. Renaming changes only this key, so comments and the rest of
the file stay as they are, and the folder is never moved: keychain secrets, history and cookies are
stored under the folder's path.

## Workspace HTTP settings (`http_client`)

The `[settings]` table of the root `folder.toml`. Request directives win over these.

```toml
[settings]
timeoutSecs = 60
connectTimeoutSecs = 5
followRedirects = true
verifyTls = true
proxy = "socks5://127.0.0.1:1080"   # "" = system proxy, "none" = direct
caCert = "certs/ca.pem"              # relative to the workspace, or absolute
clientCert = "certs/client.pem"      # PEM certificate (+ key, or see clientKey)
clientKey = "certs/client.key"
```

A PKCS#12 (`.pfx` / `.p12`) client certificate works too. Its password is a workspace secret in
the OS keychain, never in `folder.toml`.

Redirects keep secrets with their server. A request with a secret in a header doesn't follow a
redirect to another origin, and one with a secret in the body doesn't follow a 307 or 308 there
(those send the body again). Going from `http` to `https` on the same host (port 80 to 443)
counts as the same origin.

## Tags and docs (`request_meta`)

- **Tags**: `# @tag smoke, auth` lines, for filtering in the sidebar, ⌘K and
  `plumbq run --tag`.
- **Docs**: plain comment lines before the request line (not `# @…` directives) are the
  request's markdown documentation.

```http
### Create user
# Creates a user. Needs an **admin** token.
#
# Returns the new user with its `id`.
# @tag users, smoke
POST {{baseUrl}}/users
```

## Assertions (`assertions`)

An assertion is a `# @assert <subject> <operator> [value]` line:

```text
status == 200                      status in 200..299        status < 400
duration < 500                     size < 10000              (milliseconds, bytes)
header Content-Type contains json  header X-Request-Id exists
body contains "created"            body.$.user.id == 1
body.$.items.length > 0            body.$.items[0].name == "Ada"
body.$.token exists                body.$.email endsWith "@example.com"
```

Operators: `==`, `!=`, `<`, `<=`, `>`, `>=`, `contains`, `startsWith`, `endsWith`, `exists`,
`in a..b` (both ends included). Values are numbers, `"quoted"` or `'quoted'` strings, `true`,
`false`, `null`, or a bare word (a string). Body paths use the JSON path subset below. An
expression that can't be understood fails with a message that says why.

Expressions may use `{{variables}}` (`body.$.id == {{userId}}`), filled in with the values the
request was sent with. Secrets stay as their `{{name}}` placeholder, which is also what the
masked response contains, so comparisons still work and no message shows a secret.

## JSON paths (`json_path`)

Shared by assertions and chaining: `$`, `$.user.id`, `$.items[0].name`, `$["key with spaces"]`,
`$.items.length`. The response filter also takes wildcards: `$.items[*].name`, `$.users.*`.

## Chaining (`chain`)

Use a value from another request's latest response, with the syntax of the VS Code REST Client:

- `{{login.response.body.$.token}}`: a JSON path into the body,
- `{{login.response.body.*}}`: the whole body,
- `{{login.response.headers.Location}}`: a response header.

`login` names a request in the workspace: its reference name (`# @name login`, which wins), its
name (`### Login`), its file name (`auth/login.http`) or its id. The value comes from that
request's latest response in history for the same environment, so send it first. Values whose
path looks like a credential (`token`, `secret`, …) are treated as secrets and masked.

History stores responses masked. A `{{name}}` in a chained value becomes the secret again only
when Plumbq masked that secret in the response; a `{{name}}` the server wrote stays text, so a
response can't pull a secret into the next request. History keeps the first 1 MB of a body.

## Environment files (`environments`)

One file per environment in `environments/`, named `<name>.toml`. Any name works
(`my env`, `staging.eu`) except an empty one, one that starts with `.` or `$`, or one with `|`,
`/`, `\` or a control character. Names match the file name exactly (`Prod` doesn't open
`prod.toml`). A file with an unusable name (`a|b.toml`) is left out of the list; the environments
dialog and picker say so, so rename the file to use it.

## Variables from outside the environment (`dynamic`, `file_vars`)

- `{{$uuid}}`, `{{$timestamp}}`, `{{$isoTimestamp}}`, `{{$randomInt}}`: new for each send.
- `{{$prompt name}}`: asked from the user for each send (passwords, one-time codes).
- `{{$processEnv NAME}}` and `{{$env.NAME}}`: the app's OS environment variables.
- `{{$dotenv NAME}}`: a `KEY=VALUE` line in the workspace's `.env` file. Quotes around the
  value are removed; after an unquoted value, ` # text` is a comment.
- `@name = value` lines in a `.http` file are variables for every request in that file. They
  win over environment variables and may use them (`@api = {{baseUrl}}/v2`) and each other.

## Multipart bodies (`multipart`, `body_files`)

```http
POST /upload
Content-Type: multipart/form-data; boundary=WebAppBoundary

--WebAppBoundary
Content-Disposition: form-data; name="title"

Holiday
--WebAppBoundary
Content-Disposition: form-data; name="photo"; filename="beach.png"
Content-Type: image/png

< ./beach.png
--WebAppBoundary--
```

A body that is `< ./data.json`, or `< ./photo.png` lines in a multipart body, include a file. Files
are read at send time, relative to the `.http` file, and must be inside the workspace, so a
shared workspace can't read `~/.ssh/id_rsa`. Only lines written in the body load a file: a
variable whose value looks like `< ./file` is sent as text. "Copy as code" turns the parts into each client's
own multipart API.

## Postman export (`export::postman`)

- Folders and order are kept. Postman has no folder headers, so `folder.toml` headers are copied
  into each request (closer folders win, the request's own headers win).
- Root `folder.toml` variables become collection variables. Environments become separate Postman
  environment files, with secrets as empty `secret` values.
- `{{$uuid}}` becomes Postman's `{{$guid}}`. Variables Postman doesn't know (`$prompt`,
  `$processEnv`, `$dotenv`, `$auth.token`, chained responses) are left as written.
- `# @assert status == N` lines become a `pm.test`; other assertions go into the description.
  Docs comments become the request description.
- File variables (`@host = api.test`) are put into each request: Postman has no per-file scope,
  and collection variables would clash between files that use the same name.

## Bru markup (`import::bruno::lang`)

Bruno collections use top-level blocks of three kinds:

```text
meta {               dictionary: `key: value` lines, `~key` = disabled
  name: Get users
}
body:json {          text: indented free text
  {"a": 1}
}
vars:secret [        list: one name per line
  token
]
```

## Rules for sends by an AI agent (`guard`)

On top of the production check every send gets. That check, and these rules, use the method a
server may act on: a GET with `X-HTTP-Method-Override: DELETE` (or `X-HTTP-Method`,
`X-Method-Override`) counts as a DELETE. Masking keeps secret values away from the agent, but
an agent that can edit requests could still point one at its own server, so:

- A request that uses secrets (keychain secrets, `$dotenv`, `$processEnv`, OAuth tokens, chained
  responses) only goes to a host named by a URL in the chosen environment or the workspace
  defaults (e.g. `baseUrl`).
- A body that sends a file (`< ./data.json`) counts as a secret too. Files in `environments/`,
  `.env` files and files whose name comes from a secret or a response are never sent by an
  agent.
- A write (anything but GET, HEAD, OPTIONS) never goes to the host of a production environment,
  whichever environment the agent picked.
- `$processEnv` / `$env.` are off unless the server was started with them allowed: they can read
  any variable of the agent's process.
- What the user approved is what is sent. A request that changes while they are asked, or
  during an approved run, is refused instead of sent.
- A redirect to another origin on this machine, the local network or a production host is not
  followed; the redirect response comes back. Any IP address that isn't clearly public counts as
  local, and so do `localhost`, `*.localhost`, `*.internal` and `*.local`. A public-looking host
  the request reaches (its own host or a redirect hop) that resolves to such an address is
  refused too, unless an environment or the workspace defaults point at it (your own dev
  domains, like `api.myapp.test` through Valet or dnsmasq). The request's own host is not
  checked when it ends in `.test`, `.example` or `.invalid`, because nobody can register those
  names: `http://myapp.test` works through Valet or Herd with no environment. A redirect to such
  a name is checked like any other. Approving a send doesn't count: the dialog shows the name,
  not where it leads. A request whose URL has no host the app can check is refused. Hosts are
  read the way the HTTP client reads them, so `http://a.test@b.test/` counts as `b.test`. Known
  limit: behind a proxy (the workspace `proxy` setting or one from the system) the proxy looks
  up the names, so the app can't see the addresses and only the redirect rules apply. The app
  does not detect system proxies.
- `import_url` downloads only after asking, unless an environment points at the URL's host and
  that host is public and not production. Every redirect is checked the same way, and a hop the
  user didn't approve fails the download. `import_file` never reads files in `environments/` or
  `.env` files, and a file that can't be parsed is reported without quoting it.
- Text from the agent or from an imported file is shown in the approval dialog with line breaks,
  control characters and Unicode format characters (bidi overrides, zero-width characters)
  escaped, so it can't fake or reorder what the user reads.

## Runner (`runner`)

A request fails when an assertion fails, when it has no assertions and the status is 400 or
higher, or when it can't be sent. Writes to production are skipped unless allowed. With a data
file, the selection runs once per row and the row's columns are variables. A data column or
`--var` that sets a secret's name stays a secret, so it is masked like one. A report keeps the
first 10,000 results; its counts include every request.

## Re-importing (`import::write`)

Every imported request carries a `# @source METHOD url | Imported name` line. Importing the same
collection again finds the earlier import by those lines and updates it in place: new requests
are added, requests the source renamed get the new name unless the user renamed them, and
nothing the user edited (headers, body, URL, name) is overwritten or deleted.

Credentials never reach the files. Literal values in `Authorization` headers, and in headers,
query parameters, form fields and top-level JSON keys whose name looks secret (`token`, `key`,
`secret`, `password`, `session`, `csrf`, `cookie`, …), become `{{secret}}` placeholders. So does
a Postman API key wherever it lands, whatever its header or parameter is called. Secret-looking
variables become secret names. The import notes list the secrets to set. Pasted cURL commands
get the same treatment, and in the app their values are also kept as secrets of the selected
environment, so the request works at once (a secret that already exists is not changed).

## Anthropic adapter (`assistant::anthropic`)

- **Prompt caching**: an explicit breakpoint on the system prompt (tools come before it, so both
  are cached) plus automatic caching for the growing conversation. The prefix stays byte for
  byte the same: tools sorted by name, no per-turn text in `system`.
- **Thinking blocks**: current models think by default and sign their thinking blocks. Each
  assistant turn's blocks are kept as received and sent back unchanged.
- **Refusals**: models that support it get the server-side fallback; a final refusal shows in the
  chat instead of an empty answer.
