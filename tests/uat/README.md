# VS Code Extension UAT rq Fixtures

This folder is a UAT workspace for the RQ VS Code extension.

It contains a curated set of `.rq` files for manual/UAT testing of:

- Request Explorer discovery and grouping
- Environment selection
- Running requests and surfacing parse errors
- Auth provider discovery and OAuth flows (where applicable)

No real credentials are stored here. Provide values through a local `.env` file.

Keycloak:

- Start: [scripts/start-keycloak-uat.sh](scripts/start-keycloak-uat.sh)
- Stop: [scripts/stop-keycloak-uat.sh](scripts/stop-keycloak-uat.sh)

---

## Visual Test Plan

This document is a manual (visual) test plan for the RQ VS Code extension.

Primary focus:

- Authentication flows (OAuth2 implicit + authorization code with PKCE)
- Generic day-to-day usage of the Request Explorer

Out of scope:

- Language definition correctness (covered by CLI tests)
- Performance benchmarking
- Marketplace publishing and signing

### Prerequisites

This test plan assumes you have a Keycloak instance you can control (local or remote) to exercise the supported authentication providers.

Keycloak in Docker (localhost:9090)

If you don’t already have a Keycloak instance, use the repo scripts to start a local Keycloak configured for this test plan (port `9090` so `8080` remains available for the echo server):

- Start: [scripts/start-keycloak-uat.sh](scripts/start-keycloak-uat.sh)
- Stop: [scripts/stop-keycloak-uat.sh](scripts/stop-keycloak-uat.sh)

The start script imports a realm named `rq-uat` and creates clients for Authorization Code, Implicit, and Client Credentials.

If your VS Code is running in a container/remote environment, ensure `http://localhost:9090` is reachable from that environment (you may need port forwarding).

### Test data (workspace fixtures)

Use this UAT fixture workspace (this folder).

These files cover request discovery (folders/endpoints), environments/variables, imports, auth variants, and intentional error cases.

### Test matrix

Each test case includes:

- Steps
- Expected results

Repeat key flows in both modes if applicable:

- Prod mode (stable versions)
- Integration mode (dev versions like `x.y.z-dev.N`)

---

## A. Activation & CLI installation

### A1. Activation baseline (CLI already installed)

Steps:

1. Ensure `rq` CLI is installed and available to the extension.
2. Open VS Code to the test workspace.
3. Wait for activation (open any `.rq` file if needed).

Expected:

- No “rq CLI is not installed” prompt.
- RQ Request Explorer renders items.
- “RQ” output channel shows normal command execution logs when you interact.

### A2. Missing CLI (fresh machine)

Steps:

1. Ensure `rq` is not available to the extension (no working CLI binary configured).
2. Reload the VS Code window.
3. Observe the prompt.

Expected:

- A warning prompt appears indicating the CLI is missing.
- Choosing “Install Now” starts installation.
- RQ Request Explorer shows “Installing rq CLI…” placeholder while install is in progress.

### A3. Install completion refresh (no reload)

Steps:

1. Trigger CLI installation.
2. Keep VS Code open; do not reload.
3. Wait for the installer process to finish.

Expected:

- The “Installing…” placeholder disappears automatically.
- RQ Request Explorer refreshes and loads real request items.

### A4. Version mismatch (update CLI)

Steps:

1. Install an `rq` version that does not match the extension version.
2. Reload VS Code.
3. Choose “Update Now`.

Expected:

- The update starts.
- The extension clears the “installing” state when the process ends.
- The explorer refreshes after update.

---

## B. Error review in the Request Explorer

### B1. Errors in the `errors` folder

Steps:

1. Open the UAT folder in VS Code that contains the `.rq` files with errors (the `errors` subfolder).
2. Wait for the RQ Request Explorer to load and finish analyzing the files.
3. Open the Problems view in VS Code.
4. Find the two problems reported by the RQ analyzer and click each one.

Expected:

- Clicking each problem opens the corresponding `.rq` file.
- The line indicated by the problem is underlined in the editor.
- The location (line/column) matches the information shown in the Problems view.

---

## C. Using the Request Explorer to execute requests

### C1. Execute a request without selecting an environment

Steps:

1. Open the `tests/uat` folder in VS Code.
2. Open the RQ Request Explorer view.
3. Locate the `get_basic` request defined in `requests/basic.rq`.
4. Execute the request from the explorer (the “Run Request” action).

Expected:

- The request executes successfully against `http://localhost:8080/test`.
- The “RQ” output channel shows the executed command and the response.
- No new errors appear in the Problems view related to this execution.

### C2. Execute a request with an environment

Steps:

1. With the `tests/uat` folder still open, go to the RQ Request Explorer view.
2. Select the `local` environment in the explorer’s environment selector.
3. Locate the `shared_with_env` request defined in `requests/basic.rq`.
4. Execute the request from the explorer.

Expected:

- The request executes using the selected `local` environment.
- The URL used respects the environment values (for example `base_url` and `api_path`).
- The “RQ” output channel reflects the environment used and the execution completes without errors.

### C3. Execute a request with variables

Steps:

1. With the `tests/uat` folder open in VS Code, go to the RQ Request Explorer view.
2. Locate the `users.get` request under `requests/endpoints.rq`.
3. Run the request once without providing any extra variables.
4. Run the same `users.get` request again, this time providing the variable `user_id=321` when prompted (or via the variables UI).

Expected:

- The first execution uses the default value `user_id = 123` and the URL is `http://localhost:8080/users/123?v=1`.
- The second execution uses the overridden value `user_id = 321` and the URL is `http://localhost:8080/users/321?v=1`.

---

## D. OAuth authentication flows

The extension supports multiple redirect handling strategies:

- VS Code URI handler: `vscode://rq-lang.rq-language/...`
- Localhost redirect capture: `http://localhost:<port>/...`
- Manual copy/paste of the final redirect URL

These tests assume you have an OAuth provider configured in your `.rq` files and that you use the consolidated auth fixtures.

### D0. Common setup

1. Use the auth fixtures under:
   - [auth/shared.rq](auth/shared.rq)
   - [auth/basic.rq](auth/basic.rq)
2. Copy `.env.example` in this folder to a new file named `.env` and fill in all placeholder values (API_TOKEN, OAUTH_CLIENT_ID, OAUTH_CLIENT_SECRET, OAUTH_AUTHORIZATION_URL, OAUTH_TOKEN_URL, OAUTH_SCOPE, OAUTH_CC_CERT_CLIENT_ID, OAUTH_CC_CERT_PASSWORD) according to your OAuth provider and test setup.
3. Ensure your OAuth provider is configured to allow the redirect URIs used in the fixtures.

Expected:

- Each environment in shared.rq exposes an `auth_name` value that selects one auth configuration.
- The `.env` file is present and contains valid values so the variables used in shared.rq can be resolved.
- The auth config is discoverable by the extension when running requests from basic.rq.

### D1. Execute auth_basic with each environment

Steps:

1. Open the `tests/uat` folder in VS Code.
2. Open the RQ Request Explorer view.
3. Locate the `auth_basic` request defined in `auth/basic.rq`.
4. Select the `bearer_env` environment and run `auth_basic`.
5. Select the `oauth_ac_default_env` environment and run `auth_basic` again.
6. Repeat for `oauth_ac_custom_env`, `oauth_cc_env`, `oauth_cc_cert_env`, and `oauth_implicit_default_env`.

Expected:

- For each environment, the request runs using the corresponding auth configuration referenced by its `auth_name`.
- The “RQ” output channel shows that the request is protected by the expected auth type (bearer, authorization code, client credentials, client credentials with cert, or implicit).
- Any browser-based flows (authorization code or implicit) open the browser and complete successfully when valid credentials are provided.

---

## E. Regression checklist (quick)

Use this as a fast smoke test after changes:

- Activation prompts only when CLI is missing or mismatched
- Explorer shows “Installing…” only while the installer process is running
- Explorer refreshes after install/update without reload
- Environment selection persists for the session
- Run Request works against the local echo server
- OAuth callback handling works (at least one redirect mode)
