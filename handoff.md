# Handoff: fraccional-googleworkspace-mcp

## Current state

- Local clone: `~/repositories/fraccional-googleworkspace-mcp`
- Destination remote: `https://github.com/urvana/fraccional-googleworkspace-mcp.git`
- Source remote: `https://github.com/lopezjurip/haulmer-googleworkspace-mcp.git`
- The destination GitHub repository does not exist yet, or is not visible to the
  currently authenticated GitHub account.
- The clone currently contains the source branch that was checked out locally.

Remote setup:

```sh
git remote set-url upstream https://github.com/lopezjurip/haulmer-googleworkspace-mcp.git
git remote -v
```

After authenticating as an account that can create repositories under `urvana`,
create the destination repository and push:

```sh
gh auth login
gh repo create urvana/fraccional-googleworkspace-mcp --private \
  --source "$HOME/repositories/fraccional-googleworkspace-mcp" \
  --remote origin --push
```

If the repository is created manually, push the current branch with:

```sh
git push -u origin HEAD
```

## Google Workspace identity

`@urvana.net` and `@fraccionalcl` are treated as the same Google Workspace
organization/account setup. Use the primary Workspace identity for OAuth
testing, and verify in `gws auth status` which email Google returns. An alias
is not a separate Google OAuth user or a separate Cloud organization.

Before creating the OAuth app, confirm that the Google Cloud project is inside
the Workspace organization that owns the `urvana.net` account. The existing
Haulmer project is Internal to `haulmer.com` and must not be reused for this
installation.

## Google Cloud project setup

Recommended project:

- Project name: `fraccional-googleworkspace-mcp`
- Project ID: choose an available ID, for example
  `fraccional-googleworkspace-mcp`
- OAuth audience: **Internal**
- Allowed users: the `urvana.net` Workspace organization

Create/select the project:

```sh
gcloud auth login
gcloud projects create PROJECT_ID --name="fraccional-googleworkspace-mcp"
gcloud config set project PROJECT_ID
```

Enable the APIs used by the catalog:

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

Some APIs may require separate approval or additional product configuration.
Enable only the APIs needed by the final service catalog if the project owner
wants a narrower footprint.

## OAuth client

In Google Cloud Console:

1. Open **Google Auth Platform / OAuth consent screen**.
2. Select **Internal** and set the application/support details for Fraccional.
3. Configure the required scopes from `crates/core/src/scopes.rs`.
4. Create **Credentials → OAuth client ID → Desktop app**.
5. Record the client ID and secret for the implementation owner.

The app uses a loopback redirect and the `gws` CLI. Installed-app OAuth
credentials are public by design; do not treat the desktop client secret as a
server secret. Never commit service-account keys, refresh tokens, updater
private keys, or GitHub tokens.

Update the fork's `crates/core/src/oauth_client.rs` with the new Desktop client
values. Do not copy the Haulmer client ID into the Fraccional fork: it belongs
to the `haulmer-connect` project and its Internal audience.

## Workspace admin configuration

If the Workspace administrator restricts Gmail, Drive, Chat, or high-risk
scopes, either:

- enable **Trust internal apps**, or
- add this OAuth client ID as a trusted app with the required Google data
  access.

Test with the intended primary account and confirm:

```sh
gws auth status
```

The reported `user` should identify the expected Workspace user. Confirm that
the granted scopes match the selected service permissions; the alias
`@fraccionalcl` may not be the email returned by the OAuth token.

## Local isolation and pilot

Use a dedicated config folder so this fork cannot reuse another `gws` login:

```sh
APP_DATA="$HOME/Library/Application Support/com.fraccional.googleworkspace.mcp"
mkdir -p "$APP_DATA"
cat > "$APP_DATA/settings.json" <<EOF
{ "config_dir": "$HOME/.config/gws-fraccional", "key_storage": "file" }
EOF
```

The app sets `GOOGLE_WORKSPACE_CLI_CONFIG_DIR` and
`GOOGLE_WORKSPACE_CLI_KEYRING_BACKEND` for its child processes. Do not point
the fork at an existing `~/.config/gws` directory during testing.

Pilot checklist:

1. Authenticate once with the dedicated config folder.
2. Verify `gws auth status` returns the expected account.
3. Test one read-only service first, then each enabled service.
4. Test revoke/logout and verify that no credentials remain in the dedicated
   folder.
5. Keep the Haulmer and Fraccional OAuth projects and credential folders
   separate.

## Next implementation steps

1. Create the `urvana/fraccional-googleworkspace-mcp` GitHub repository and
   push this clone.
2. Create the Fraccional Google Cloud project and Desktop OAuth client.
3. Replace the OAuth constants in `crates/core/src/oauth_client.rs`.
4. Run the existing checks from `README.md`.
5. Perform the real pilot login with the `urvana.net` Workspace account.
