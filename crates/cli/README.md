# plumbq (CLI)

Runs the requests of a Plumbq workspace and checks their assertions, from a terminal or
CI. It uses the same core as the desktop app, so variables, folder headers, environments,
keychain secrets and the production safety rule all work the same way.

```bash
cargo build -p plumbq-cli --release
plumbq run path/to/workspace --env staging
plumbq run path/to/workspace/users            # one folder
plumbq run path/to/workspace/users/list.http  # one file
plumbq run path/to/workspace --data users.csv  # once per CSV row, columns as {{variables}}
plumbq docs path/to/workspace --output docs.html
```

The workspace root is found from the path you give: the highest folder at or above it with a
`folder.toml` or an `environments/` directory holding `.toml` files, without leaving the git
repository it's in. Pass `--root` when that guesses wrong. Requests run one at a time, in the
order the app shows them.

## Options

| Option                      | What it does                                                   |
| --------------------------- | -------------------------------------------------------------- |
| `--env`, `-e NAME`          | Environment for `{{variables}}`.                               |
| `--bail`                    | Stop at the first failing request.                             |
| `--reporter pretty`         | Readable summary (default).                                    |
| `--reporter junit`          | JUnit XML for GitHub Actions, GitLab, Jenkins and others.      |
| `--reporter json`           | Machine-readable results.                                      |
| `--output`, `-o FILE`       | Write the report to a file; the terminal still gets a summary. |
| `--prompt NAME=VALUE`       | Answer a `{{$prompt NAME}}` variable. Repeatable.              |
| `--var NAME=VALUE`          | Set a variable; wins over environments and `--data` rows.      |
| `--root DIR`                | The workspace folder, when it isn't found from the target.     |
| `--allow-production-writes` | Send POST/PUT/PATCH/DELETE… to production environments.        |
| `--no-history`              | Don't keep responses or cookies; both still work within a run. |
| `--no-cookies`              | Don't send or store cookies.                                   |
| `--tag NAME`                | Only requests tagged `# @tag NAME`. Repeatable.                |
| `--data FILE`               | CSV (header row) or JSON array: one run per row, as variables. |
| `--iterations N`            | Run the selection N times (without `--data`).                  |
| `--delay MS`                | Wait between requests.                                         |

`plumbq docs <workspace> [--env NAME] [--output docs.html]` writes a single self-contained
HTML page documenting every request (docs comments, headers, body, assertions, saved examples).
The app's Runner uses the same runner, so results match.

## Pass or fail

A request **fails** when one of its assertions fails, when it has no assertions and the status
is 400 or higher, or when it can't be sent (connection error, unknown variable, missing
`--prompt` value). Writes to a production environment are **skipped** unless
`--allow-production-writes` is given.

A request whose response couldn't be recorded in history (e.g. a read-only home folder) still
passes; the report shows a warning.

Exit code: `0` everything passed, `1` something failed, `2` usage or workspace error, including
an unknown `--env`.

## Secrets in CI

There's no keychain on CI machines. A secret named `token` in the environment is read from the
environment variable `PLUMBQ_SECRET_TOKEN` instead (the name upper-cased, anything but letters
and digits as `_`; `api-key` becomes `PLUMBQ_SECRET_API_KEY`). It's used on any machine where
it's set, before the keychain. Values stay masked in reports like any secret.

A `--var` that sets a secret's name stays a secret, and so does one whose name looks like a
credential (`API_TOKEN`, `DB_PASSWORD`): both are masked in history and output.

## Assertions

Write them above the request line:

```http
### Create user
# @assert status == 201
# @assert duration < 500
# @assert header Content-Type contains json
# @assert body.$.id exists
# @assert body.$.roles.length > 0
POST {{baseUrl}}/users
Content-Type: application/json

{"name": "Ada"}
```

Subjects: `status`, `duration` (ms), `size` (bytes), `header <Name>`, `body`, `body.$.path`
(`$.a.b`, `$.items[0]`, `$["key with spaces"]`, `.length`). Operators: `==`, `!=`, `<`, `<=`,
`>`, `>=`, `contains`, `startsWith`, `endsWith`, `exists`, `in 200..299`.

## GitHub Actions

```yaml
- run: plumbq run api --env ci --reporter junit --output api-results.xml
  env:
    PLUMBQ_SECRET_TOKEN: ${{ secrets.API_TOKEN }}
- uses: mikepenz/action-junit-report@v5
  if: always()
  with:
    report_paths: api-results.xml
```
