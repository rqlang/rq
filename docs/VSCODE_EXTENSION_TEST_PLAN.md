# VS Code Extension — Visual Test Plan

This document is a manual (visual) test plan for the RQ VS Code extension.

Primary focus:

- Authentication flows (OAuth2 implicit + authorization code with PKCE)
- Generic day-to-day usage of the Request Explorer

Out of scope:

- Language definition correctness (covered by CLI tests)
- Performance benchmarking
- Marketplace publishing and signing

## Prerequisites

This test plan assumes you have a Keycloak instance you can control (local or remote) to exercise the supported authentication providers.

Keycloak in Docker (localhost:9090)

If you don’t already have a Keycloak instance, use the repo scripts to start a local Keycloak configured for this test plan (port `9090` so `8080` remains available for the echo server):

- Start: [../tests/uat/scripts/start-keycloak-uat.sh](../tests/uat/scripts/start-keycloak-uat.sh)
- Stop: [../tests/uat/scripts/stop-keycloak-uat.sh](../tests/uat/scripts/stop-keycloak-uat.sh)

The start script imports a realm named `rq-uat` and creates clients for Authorization Code, Implicit, and Client Credentials.

If your VS Code is running in a container/remote environment, ensure `http://localhost:9090` is reachable from that environment (you may need port forwarding).

Keycloak setup requirements:

- Keycloak is running and reachable from your VS Code environment.
- Create a dedicated realm (for example `rq-uat`).
- Create OIDC clients that cover the supported auth types:
   - Authorization code + PKCE: a client with Standard Flow enabled, PKCE enabled (S256), and redirect URIs that include `vscode://rq-lang.rq-language/oauth-callback` and/or a localhost callback you will use for redirect capture.
   - Implicit: a client with Implicit Flow enabled and matching redirect URIs.
   - Client credentials: a confidential client with Service Accounts enabled; note the client secret.
- Create at least one test user in the realm (username + password) for interactive flows.
- Record the realm endpoints to use in the `.rq` auth fixtures:
   - Authorization endpoint URL
   - Token endpoint URL

Extension/CLI requirements:

- VS Code installed
- The extension installed (dev or prod build)
- `rq` CLI available (or intentionally missing, for install tests)
- GitHub CLI (`gh`) authenticated if you are testing dev/integration installers

## Test data (workspace fixtures)

Use the committed UAT fixture workspace under:

- [../tests/uat](../tests/uat)

These files cover request discovery (folders/endpoints), environments/variables, imports, auth variants, and intentional error cases.

If you want deterministic request execution during tests, run an echo server on `http://localhost:8080` (same setup as the CLI tests; see [docs/LOCAL_DEVELOPMENT.md](docs/LOCAL_DEVELOPMENT.md)).

## Observability

When running tests, keep these visible:

- VS Code Output panel → channel “RQ”
- Problems panel
- RQ Request Explorer view

## Test matrix

Each test case includes:

- Steps
- Expected results

Repeat key flows in both modes if applicable:

- Prod mode (stable versions)
- Integration mode (dev versions like `x.y.z-dev.N`)

---

# A. Activation & CLI installation

## A1. Activation baseline (CLI already installed)

Steps:

1. Ensure `rq` is installed and on PATH (or configured via `rq.cli.path`).
2. Open VS Code to the test workspace.
3. Wait for activation (open any `.rq` file if needed).

Expected:

- No “rq CLI is not installed” prompt.
- RQ Request Explorer renders items.
- “RQ” output channel shows normal command execution logs when you interact.

## A2. Missing CLI (fresh machine)

Steps:

1. Ensure `rq` is not available:
   - Remove from PATH, and clear `rq.cli.path`.
2. Reload VS Code window.
3. Observe the prompt.

Expected:

- A warning prompt appears indicating the CLI is missing.
- Choosing “Install Now” starts installation.
- RQ Request Explorer shows “Installing rq CLI…” placeholder while install is in progress.

## A3. Install completion refresh (no reload)

Steps:

1. Trigger CLI installation.
2. Keep VS Code open; do not reload.
3. Wait for the installer process to finish.

Expected:

- The “Installing…” placeholder disappears automatically.
- RQ Request Explorer refreshes and loads real request items.

## A4. Version mismatch (update path)

Steps:

1. Install an `rq` version that does not match the extension version.
2. Reload VS Code.
3. Choose “Update Now”.

Expected:

- Update starts.
- The extension clears the “installing” state when the process ends.
- Explorer refreshes after update.

---

# B. Request Explorer (generic usage)

## B1. Explorer loads requests

Steps:

1. Open the “RQ Request Explorer” view.
2. Expand the tree.

Expected:

- Environment row is shown at the top.
- Requests are grouped by folder/endpoint.

## B2. Environment selection

Steps:

1. In the Explorer, select an environment (e.g. `local`).
2. Confirm the selected environment is reflected.

Expected:

- The environment row shows the chosen environment.
- Executed requests use that environment.

## B3. Run request

Steps:

1. Start the local echo server (see above).
2. In the Explorer, run `ping`.

Expected:

- Request executes successfully.
- Output channel includes the executed command and response.
- Problems panel is not polluted with unrelated errors.

## B4. Run with variables

Steps:

1. Run “Run with Variables” on a request.
2. Enter one or more overrides using `key=value`.

Expected:

- The request is executed using the override values.
- Invalid input format is rejected with a clear prompt.

## B5. Navigation to request definition

Steps:

1. Click a request in the Explorer.

Expected:

- The corresponding `.rq` file opens.
- Cursor jumps to the request definition.

## B6. Refresh

Steps:

1. Add or edit a request in a `.rq` file.
2. Use the refresh action.

Expected:

- Explorer updates to reflect the file changes.

---

# C. Diagnostics & error surfacing

## C1. Syntax error surfaces in Problems

Steps:

1. Introduce a syntax error in a `.rq` file.
2. Trigger parsing (refresh explorer or run request).

Expected:

- A diagnostic appears in Problems with file/line/column.
- Explorer continues to function (best-effort) for valid files.

---

# D. OAuth authentication flows

The extension supports multiple redirect handling strategies:

- VS Code URI handler: `vscode://rq-lang.rq-language/...`
- Localhost redirect capture: `http://localhost:<port>/...`
- Manual copy/paste of the final redirect URL

These tests assume you have an OAuth provider configured in your `.rq` files.

## D0. Common setup

1. Use the auth fixtures under:
   - [../tests/uat/auth](../tests/uat/auth)
2. Ensure your OAuth provider is configured to allow the redirect URI you will test.

Expected:

- The auth config is discoverable by `RQ: Get Token`.

## D1. Get Token command (happy path)

Steps:

1. Run command “RQ: Get Token”.
2. Choose environment (if prompted).
3. Select an auth provider.

Expected:

- Browser opens to login/consent.
- Token is acquired and the command completes.

## D2. VS Code URI handler redirect

Steps:

1. Configure `redirect_uri` as `vscode://rq-lang.rq-language/oauth-callback` (or omit it if your CLI/extension defaults to that).
2. Run “RQ: Get Token”.
3. Complete login.
4. When the browser prompts to open VS Code, accept.

Expected:

- VS Code receives the callback.
- Token acquisition completes without manual URL copy/paste.

## D3. Localhost redirect capture

Steps:

1. Configure `redirect_uri` to `http://localhost:3000/callback` (example).
2. Run “RQ: Get Token”.
3. Complete login.

Expected:

- The extension starts a temporary local server.
- After redirect, a success page appears (or equivalent).
- The local server stops after completion.

## D4. Manual redirect handling

Steps:

1. Configure `redirect_uri` to a non-local, non-VS Code URL (example: `https://example.com/callback`).
2. Run “RQ: Get Token”.
3. Complete login until the final redirect URL is shown.
4. Copy the browser URL and paste it when VS Code asks.

Expected:

- The extension parses the pasted URL.
- Token acquisition completes.

## D5. Clear OAuth cache

Steps:

1. Acquire a token.
2. Run “RQ: Clear OAuth Cache”.
3. Run “RQ: Get Token” again.

Expected:

- A new login flow is required.

## D6. Cancel flows

Run each flow and cancel at different stages:

- Close the browser early
- Dismiss VS Code prompts
- Deny consent in provider UI

Expected:

- The extension shows a clear error message.
- No token is cached.
- The extension remains usable.

---

# E. Regression checklist (quick)

Use this as a fast smoke test after changes:

- Activation prompts only when CLI missing/mismatched
- Explorer shows “Installing…” only while installer process is running
- Explorer refreshes after install/update without reload
- Environment selection persists for the session
- Run request works against local echo server
- OAuth callback handling works (at least one redirect mode)
