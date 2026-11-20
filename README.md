# haulmer-googleworkspace-mcp

`haulmer-googleworkspace-mcp` is a Tauri desktop app for granting local AI agents access to
Google Workspace and selected Google business APIs. It runs locally on macOS and
Windows; tokens stay on the user's machine.

The app delegates OAuth, token storage, refresh and API calls to the pinned
[Google Workspace CLI](https://github.com/googleworkspace/cli) (`gws`).
The shared OAuth client is a Google **Desktop** client with an **Internal**
audience, restricted to Haulmer accounts.

## Repository layout

| Path | Purpose |
|---|---|
| `crates/core` | Core behaviour, OAuth scopes, status and revocation |
| `src-tauri` | Tauri v2 shell, tray, IPC commands and packaging |
| `src` | React 19 + TypeScript + Tailwind CSS UI |
| `docs` | Product specification, research and implementation tickets |
| `.github/workflows` | CI and signed updater release workflows |

## Prerequisites

- Node.js 24
- pnpm 12
- Rust stable 1.90 or newer, and Cargo
- Tauri v2 system prerequisites:
  <https://v2.tauri.app/start/prerequisites/>
- Google Cloud CLI (`gcloud`) for project administration

On macOS, install the toolchains with Homebrew if needed:

```sh
brew install node pnpm rustup-init google-cloud-sdk
rustup-init
```

On Windows, install Node.js, pnpm, Rust through `rustup` and the Google Cloud
CLI using their official installers. Tauri also requires the Visual Studio C++
build tools and WebView2.

## Development

Install dependencies:

```sh
pnpm install
```

Run the frontend dev server only:

```sh
pnpm dev
```

Run the complete desktop application with hot reload:

```sh
pnpm tauri dev
```

Run the checks used by CI:

```sh
pnpm lint                 # Biome + Clippy
cargo fmt --all -- --check
cargo test --workspace --locked
pnpm build                # TypeScript check + Vite production build
```

`pnpm lint:ts` runs Biome for TypeScript/TSX. `pnpm lint:rust` runs
`cargo clippy --workspace --all-targets -- -D warnings`.

## Google Cloud setup

The current OAuth client lives in the renamed Google Cloud project:

- Project ID: `fraccional-gws-mcp`
- Project name: `haulmer-googleworkspace-mcp`
- Project number: `252304543508`
- OAuth audience: **Internal**
- Allowed users: `@haulmer.com`

Authenticate and select the project:

```sh
gcloud auth login
gcloud config set project fraccional-gws-mcp
gcloud auth application-default login
```

Enable the APIs used by the catalog. This command is safe to rerun:

```sh
gcloud services enable \
  gmail.googleapis.com \
  calendar-json.googleapis.com \
  chat.googleapis.com \
  meet.googleapis.com \
  drive.googleapis.com \
  docs.googleapis.com \
  sheets.googleapis.com \
  slides.googleapis.com \
  forms.googleapis.com \
  tasks.googleapis.com \
  people.googleapis.com \
  admin.googleapis.com \
  cloudidentity.googleapis.com \
  youtube.googleapis.com \
  googleads.googleapis.com \
  analyticsadmin.googleapis.com \
  analyticsdata.googleapis.com \
  searchconsole.googleapis.com \
  content.googleapis.com
```

Some APIs, especially Business Profile, may require separate Google approval or
an additional project configuration. The UI marks unavailable services without
blocking the rest of the catalog.

### OAuth consent screen and client

In Google Cloud Console:

1. Open **APIs & Services → OAuth consent screen**.
2. Select **Internal**.
3. Set the application name, support email and developer contact.
4. Confirm the OAuth scopes needed by the catalog are available.
5. Open **Credentials → Create credentials → OAuth client ID**.
6. Select **Desktop app**.
7. Keep the client ID aligned with
   `crates/core/src/oauth_client.rs`.

The Desktop client ID is included in the application by design. Google treats
installed apps as public clients: the token exchange uses the client ID and
PKCE, without a client secret. Do not put the updater private key, GitHub
tokens or any server credentials in the repo.

For Google Workspace, the Google Cloud OAuth app must be trusted by the
organization's admin if the Admin Console requires explicit trust for internal
apps.

### Local OAuth isolation

By default the app uses the `gws` default folder (`~/.config/gws`), with the
encryption key in the OS keyring. If you already use `gws` on your machine and
want to keep that login apart, give the app its own folder in its settings file
before you start it:

```sh
# macOS
APP_DATA="$HOME/Library/Application Support/com.haulmer.googleworkspace.mcp"
mkdir -p "$APP_DATA"
cat > "$APP_DATA/settings.json" <<EOF
{ "config_dir": "$PWD/.local/gws", "key_storage": "auto" }
EOF
pnpm tauri dev
```

On Windows the file is `%APPDATA%\com.haulmer.googleworkspace.mcp\settings.json`.
With a custom folder, `"auto"` key storage keeps the encryption key in
`.encryption_key` inside that folder. `.local/` is ignored by git.

Do not export `GOOGLE_WORKSPACE_CLI_CONFIG_DIR` or
`GOOGLE_WORKSPACE_CLI_KEYRING_BACKEND` for this. The app sets both for every
`gws` process from its settings and ignores the values in your shell.

The development build and the installed app share this settings file. To go
back to the default folder, delete `settings.json`, or set `"config_dir"` to
`null`. Never use a real user's default credentials directory in tests.

## MCP integration

The installed app can run as an MCP server:

```sh
/Applications/haulmer-googleworkspace-mcp.app/Contents/MacOS/haulmer-googleworkspace-mcp mcp
```

### Connect an agent

Click the tray item **Conectar un agente…**. A window of the same name opens. It
works with no Google account connected, and it has two cards.

**Card 1, "Instalación en un clic"** (Chat apps such as Claude Desktop):

1. Click **Instalar MCP**.
2. Your AI app opens (for example Claude Desktop). Click "Instalar" there.

The app builds `haulmer-googleworkspace-mcp.mcpb`, saves it in its data folder and opens it
with the default app. The OS hands the file to a compatible Chat app, which
shows its own install dialog. If no app can open `.mcpb` files, the window says
so ("No encontré una app que abra extensiones MCP. Usa el prompt de abajo.").

The `.mcpb` is a thin bundle: a `manifest.json` (in Spanish), the app icon and a
placeholder `README.md`. It does not contain the server. Its command points at
the installed app, with `args: ["mcp"]`.

**Card 2, "Con un prompt (Claude Code, Codex y otros agentes)"**:

1. Click **Copiar** to copy the install prompt.
2. Paste the prompt into your agent (Claude Code, Codex, Hermes, Cursor).

The prompt contains the absolute path of the app, so the agent can:

- add the MCP server and check it (`claude mcp add` and `claude mcp get` for
  Claude Code, `codex mcp add` and `codex mcp get` for Codex, a generic stdio
  entry for other agents);
- install the core `gws` service skills (the "Services" table of gws
  `docs/skills.md`) with `npx skills add` (optional; if `npx` fails, the agent
  says so);
- tell you the manual steps for ChatGPT desktop. For Claude Desktop, it points
  you to **Instalar MCP**.

On macOS, if the app runs from a translocated path (`/AppTranslocation/`), the
window shows only an error. Move the app to Applications and open it again. If
the app runs from the mounted DMG (`/Volumes/`), the window shows a warning and
the prompt. The same checks apply to the `.mcpb`.

The app never writes agent config files or agent skill folders. Without `npx`,
get the skills from the ZIP: **Abrir skills › Descargar .zip** in the Skills
window.

Web chats (claude.ai, chatgpt.com) cannot use a local MCP server at all.

Not verified yet: whether Claude Desktop accepts a thin bundle whose command
points outside the bundle. If it does not, the fallback is to put the binary
inside the bundle.

### Manual configuration

Use this only if you configure a client by hand. For Claude Code and Codex:

```sh
claude mcp add --scope user haulmer-googleworkspace-mcp -- "<path-to-haulmer-googleworkspace-mcp>" mcp
claude mcp get haulmer-googleworkspace-mcp

codex mcp add haulmer-googleworkspace-mcp -- "<path-to-haulmer-googleworkspace-mcp>" mcp
codex mcp get haulmer-googleworkspace-mcp
```

For Claude Desktop, use **Instalar MCP** instead. If you prefer to edit its
configuration by hand (Edit Config):

```jsonc
// ~/Library/Application Support/Claude/claude_desktop_config.json
{
  "mcpServers": {
    "haulmer-googleworkspace-mcp": {
      "command": "<path-to-haulmer-googleworkspace-mcp>",
      "args": ["mcp"]
    }
  }
}
```

```toml
# ChatGPT desktop, Codex CLI and IDE extensions:
# ~/.codex/config.toml
# In ChatGPT desktop you can also use Settings > MCP servers.
[mcp_servers.haulmer-googleworkspace-mcp]
command = "<path-to-haulmer-googleworkspace-mcp>"
args = ["mcp"]
```

### Tools

- `gws`: runs a `gws` command. `auth` commands are refused over MCP.
- `status`: read-only, no arguments. Reports the connected account, the session
  (valid or expired), whether `gws` is installed, the services the app may use
  (with "Lectura" or "Lectura y Escritura") and the ones it may not, the business
  connectors, and the next step to activate more services. Models should call it
  first.
- `open_haulmer_googleworkspace_mcp`: opens the desktop app on its settings window.

It is best to authenticate in the desktop app first, but you do not have to.
When a `gws` call fails, the server answers in Spanish with the next step. It
names the exact button (**Abrir configuración**, **Conectar** or **Guardar**)
for a missing `gws`, no account, an expired session or a missing permission, and
it suggests `open_haulmer_googleworkspace_mcp`.

The app allows one instance. A second launch, including `open_haulmer_googleworkspace_mcp`,
shows the settings window instead of starting a second tray icon.

## Releases and updater signatures

The release workflow in `.github/workflows/release.yml` builds:

- Windows x64 NSIS: `.exe`
- macOS Apple Silicon: `.dmg` and updater `.app.tar.gz`
- macOS Intel: `.dmg` and updater `.app.tar.gz`
- SHA-256 files, `SHA256SUMS.txt` and `latest.json`

The updater artifacts are signed with a Tauri minisign-compatible key. This is
different from OS code signing: the current pilot releases are **not** signed
with an Apple Developer ID certificate or a Windows Authenticode certificate.

### Generate the updater key once

Keep the private key outside the repository:

```sh
pnpm tauri signer generate -w ~/.tauri/haulmer-googleworkspace-mcp.key
```

The command writes a private key and prints the public key. Put only the public
key in `src-tauri/tauri.conf.json` under `plugins.updater.pubkey`. Never commit
the private key. Losing it prevents existing installations from accepting
future updates.

For a local signed build on macOS/Linux:

```sh
export TAURI_SIGNING_PRIVATE_KEY="$HOME/.tauri/haulmer-googleworkspace-mcp.key"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="<key-password>"
pnpm tauri build
```

On PowerShell:

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY="$HOME\.tauri\haulmer-googleworkspace-mcp.key"
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD="<key-password>"
pnpm tauri build
```

The workflow requires these GitHub Actions secrets:

- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

Configure them under **Settings → Secrets and variables → Actions**. The private
key can be provided as its contents or as a path understood by the Tauri CLI;
the workflow uses the secret contents.

### Create a release

1. Update the version consistently in:
   - `package.json`
   - workspace `Cargo.toml`
   - `src-tauri/tauri.conf.json`
2. Run the local checks:

   ```sh
   pnpm install --frozen-lockfile
   pnpm lint
   cargo fmt --all -- --check
   cargo test --workspace --locked
   pnpm build
   ```

3. Commit and merge the version change to `main`.
4. Create and push a matching tag:

   ```sh
   git tag v0.1.0
   git push origin main
   git push origin v0.1.0
   ```

   The workflow rejects a tag that does not exactly match
   `src-tauri/tauri.conf.json.version`.
5. Open the **Release** workflow in GitHub and wait for all three platform
   builds.
6. Review the generated draft release and publish it.

The updater endpoint is:

```text
https://github.com/lopezjurip/haulmer-googleworkspace-mcp/releases/latest/download/latest.json
```

Verify installer downloads:

```sh
shasum -a 256 -c haulmer-googleworkspace-mcp_*.dmg.sha256
```

On PowerShell:

```powershell
Get-FileHash .\haulmer-googleworkspace-mcp_*setup.exe -Algorithm SHA256
```

Because pilot installers are not OS-code-signed, macOS may require
**right-click → Open** the first time and Windows SmartScreen may require
**More info → Run anyway**.
