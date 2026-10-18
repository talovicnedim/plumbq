# Releasing

A release is a git tag. Pushing `v0.1.0` runs `.github/workflows/release.yml`, which builds
installers for macOS, Windows and Linux into a **draft** GitHub release. Nothing is published
until you publish the draft.

Signing, notarization and auto-updates are opt-in through repository secrets. Without them the
workflow still succeeds: builds are unsigned and have no updater.

## What a release contains

| Platform | Files                                                      |
| -------- | ---------------------------------------------------------- |
| macOS    | `Plumbq_<version>_universal.dmg` (Apple Silicon and Intel) |
| Windows  | `.msi` and NSIS `-setup.exe`                               |
| Linux    | `.AppImage`, `.deb` and `.rpm`                             |

Every bundle contains the app plus two sidecars next to its executable: `plumbq-mcp` (the
MCP server for AI apps, in **AI → AI apps**) and `plumbq-cli` (the `plumbq run` runner).
With auto-updates on, each platform also gets a signed update archive (`.app.tar.gz`,
`.nsis.zip`, `.AppImage.tar.gz` plus `.sig`) and the release gets `latest.json`.

## 1. Auto-updates (recommended)

The app checks `https://github.com/talovicnedim/plumbq/releases/latest/download/latest.json`
once per launch and offers "Restart to update". Updates are signed with your own key pair.

1. Generate the keys (keep the private key and its password safe; losing them means existing
   installs can't verify future updates):

   ```bash
   cd apps/desktop
   pnpm tauri signer generate -w ~/.tauri/plumbq.key
   ```

2. Put the **public** key (the `.pub` file's content) in
   `apps/desktop/src-tauri/tauri.updater.conf.json` as `plugins.updater.pubkey`, replacing the
   `REPLACE_WITH_…` placeholder, and commit it. It's public by design.
3. Add repository secrets (Settings → Secrets and variables → Actions):
   - `PLUMBQ_UPDATE_KEY`: the content of `~/.tauri/plumbq.key`
   - `PLUMBQ_UPDATE_KEY_PASSWORD`: its password

   The workflow passes them to Tauri as `TAURI_SIGNING_PRIVATE_KEY` and
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, the only names Tauri reads.

The workflow merges `tauri.updater.conf.json` only when the private key secret exists **and** the
placeholder is gone; otherwise it warns "Auto-updates are off for this build". Dev builds never
have the updater: the plugin is only registered when that config is present.

Releases are found by the updater only once they are **published** (drafts aren't "latest").

## 2. macOS signing and notarization

Needs a paid Apple Developer account. Without it, macOS blocks the first launch: users open
System Settings > Privacy & Security and click **Open Anyway** (right-click → Open no longer
works since macOS 15), or run:

```bash
xattr -dr com.apple.quarantine "/Applications/Plumbq.app"
```

To sign and notarize:

1. In the Apple Developer portal create a **Developer ID Application** certificate, install it in
   Keychain Access, and export it (with its private key) as a `.p12` with a password.
2. Create an app-specific password at appleid.apple.com (Sign-In and Security).
3. Add repository secrets:
   - `APPLE_CERTIFICATE`: `base64 -i certificate.p12 | pbcopy`
   - `APPLE_CERTIFICATE_PASSWORD`: the `.p12` password
   - `APPLE_SIGNING_IDENTITY`: e.g. `Developer ID Application: Your Name (TEAMID)`
     (`security find-identity -v -p codesigning`)
   - `APPLE_ID`: your Apple ID email
   - `APPLE_PASSWORD`: the app-specific password
   - `APPLE_TEAM_ID`: your team ID

Tauri signs the external binaries (`plumbq-mcp`, `plumbq-cli`) together with the app and
notarizes the result, so the bundled MCP server and CLI are trusted too.

## 3. Windows signing (optional)

Unsigned installers trigger SmartScreen ("Windows protected your PC" → More info → Run anyway).
To sign with an OV/EV code-signing certificate exported as `.pfx`:

- `WINDOWS_CERTIFICATE`: `base64` of the `.pfx`
- `WINDOWS_CERTIFICATE_PASSWORD`: its password

The workflow imports it, passes its thumbprint to the bundler and timestamps with DigiCert.
(Cloud signing services such as Azure Trusted Signing need a custom `signCommand` instead.)

## 4. Cut the release

1. Bump the version in `Cargo.toml` (`[workspace.package] version`),
   `apps/desktop/src-tauri/tauri.conf.json` (`version`) and `apps/desktop/package.json`. The
   workflow stops right away when the tag (`v0.1.0`) doesn't match this version.
2. Run `pnpm check`.
3. Commit, then tag and push:

   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```

4. Wait for the three jobs, then check the draft release.

## 5. Verify before publishing

- Download the macOS `.dmg`, open it and check the sidecars:

  ```bash
  ls "/Volumes/Plumbq/Plumbq.app/Contents/MacOS"
  # Plumbq  plumbq-cli  plumbq-mcp
  codesign --verify --deep --strict "/Volumes/Plumbq/Plumbq.app"   # when signed
  ```

- Linux: `./Plumbq*.AppImage --appimage-extract` and look in `squashfs-root/usr/bin/`.
- Windows: the installed folder (`%LOCALAPPDATA%\Plumbq` for NSIS) contains
  `plumbq-mcp.exe` and `plumbq-cli.exe`.
- Launch the app, open **AI → AI apps**: it should not ask you to build the server first
  (`cargo build -p plumbq-mcp`).
- With updates on, `latest.json` is attached and lists every platform.

Then publish the draft.

## Building locally

```bash
pnpm build                                   # host platform, release config, unsigned
pnpm --filter desktop tauri build --debug --bundles app --config src-tauri/tauri.release.conf.json
```

To try updater artifacts locally, export `TAURI_SIGNING_PRIVATE_KEY` (and its password) and add
`--config src-tauri/tauri.updater.conf.json`.
