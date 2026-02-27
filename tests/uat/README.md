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
2. Copy `.env.example` in this folder to a new file named `.env`

Expected:

- Each environment in shared.rq exposes an `auth_name` value that selects one auth configuration.
- The `.env` file is present and contains valid values so the variables used in shared.rq can be resolved.
- The auth config is discoverable by the extension when running requests from basic.rq.

### D1. Bearer token auth (bearer_auth)

This auth uses a static bearer token provided by the `api_token` variable. No interactive browser flow is required.

Steps:

1. Open the `tests/uat` workspace in VS Code.
2. In the RQ Request Explorer, select the `bearer_env` environment (defined in `auth/shared.rq`).
3. Locate the `auth_basic` request under `auth/basic.rq`.
4. Run the `auth_basic` request from the explorer.
5. After the request completes, open the request/response details and inspect the **Request headers** section.

Expected:

- The request executes without any OAuth login prompts.
- The request is sent to `http://localhost:8080/test`.
- The **Authorization** header is present with a `Bearer` token value derived from `api_token`.

### D2. OAuth2 authorization code (default redirect) (oauth_ac_default)

This auth performs an OAuth2 Authorization Code flow with PKCE using the extension's default redirect URI (`vscode://rq-lang.rq-language/oauth-callback`).

Steps:

1. In the RQ Request Explorer, select the `oauth_ac_default_env` environment.
2. Locate the `auth_basic` request under `auth/basic.rq`.
3. Run the `auth_basic` request.
4. When the browser opens, complete the login and consent flow in your OAuth provider.
5. When prompted, allow the browser to open VS Code so the extension can capture the callback.
6. After the request finishes, inspect the **Request headers** for the executed request.

Expected:

- The extension opens a browser window and guides you through the OAuth2 Authorization Code flow.
- After successful login, the request is executed automatically.
- The **Authorization** header is present with a `Bearer` access token obtained from the OAuth provider.

### D3. OAuth2 authorization code (custom localhost redirect) (oauth_ac_custom)

This auth performs an OAuth2 Authorization Code flow with PKCE using a custom `redirect_uri` pointing to a localhost HTTP endpoint.

Steps:

1. In the RQ Request Explorer, select the `oauth_ac_custom_env` environment.
2. Locate the `auth_basic` request under `auth/basic.rq`.
3. Run the `auth_basic` request.
4. When the browser opens, complete the login and consent flow.
5. Wait for the browser to redirect to the configured localhost callback and show the success page.
6. After the request completes in VS Code, inspect the **Request headers** for the executed request.

Expected:

- The extension starts a temporary local HTTP listener to capture the OAuth redirect.
- After successful login, the listener captures the authorization code and the extension exchanges it for a token.
- The **Authorization** header is present with a `Bearer` access token obtained from the OAuth provider.

### D4. OAuth2 authorization code (external redirect) (oauth_ac_external)

This auth performs an OAuth2 Authorization Code flow with PKCE using an external HTTPS redirect URI hosted by your identity provider (for example `https://example.com/callback`). The extension cannot listen on this domain, so the final redirect URL must be copied manually from the browser.

Steps:

1. In your auth fixtures, configure an Authorization Code auth named `oauth_ac_external` with a `redirect_uri` pointing to an external HTTPS URL (for example `https://example.com/callback`).
2. Configure an environment (for example `oauth_ac_external_env`) whose `auth_name` points to `oauth_ac_external`.
3. In the RQ Request Explorer, select the `oauth_ac_external_env` environment.
4. Locate the `auth_basic` request under `auth/basic.rq`.
5. Run the `auth_basic` request.
6. When the browser opens, complete the login and consent flow.
7. After the provider redirects to the external callback page, copy the full redirect URL from the browser address bar.
8. When prompted by the extension, paste the copied redirect URL into VS Code so it can extract the authorization code.
9. Wait for the request execution to finish in VS Code and inspect the **Request headers**.

Expected:

- The extension opens a browser window and guides you through the OAuth2 Authorization Code flow against the external redirect.
- After pasting the final redirect URL into VS Code, the extension exchanges the authorization code for an access token.
- The request to `http://localhost:8080/test` completes successfully.
- The **Authorization** header is present with a `Bearer` access token obtained from the OAuth provider.

### D5. OAuth2 client credentials (shared secret) (oauth_cc)

This auth uses the OAuth2 Client Credentials flow with a client secret (no interactive browser).

Steps:

1. In the RQ Request Explorer, select the `oauth_cc_env` environment.
2. Locate the `auth_basic` request under `auth/basic.rq`.
3. Run the `auth_basic` request.
4. After the request completes, inspect the **Request headers** for the executed request.

Expected:

- The extension does not open a browser window; it performs a backend token request using `client_id`, `client_secret`, and `token_url`.
- The request to `http://localhost:8080/test` succeeds.
- The **Authorization** header is present with a `Bearer` access token obtained via the client credentials flow.

### D6. OAuth2 client credentials with client certificate (oauth_cc_cert)

This auth uses the OAuth2 Client Credentials flow with a client certificate (`.p12` file) instead of a shared secret.

Steps:

1. In the RQ Request Explorer, select the `oauth_cc_cert_env` environment.
2. Ensure the certificate file path and password variables referenced in `auth/shared.rq` are valid for your setup.
3. Locate the `auth_basic` request under `auth/basic.rq`.
4. Run the `auth_basic` request.
5. After the request completes, inspect the **Request headers** for the executed request.

Expected:

- The extension obtains an access token from the OAuth provider using the configured client certificate.
- The request to `http://localhost:8080/test` succeeds.
- The **Authorization** header is present with a `Bearer` access token obtained via the certificate-based client credentials flow.

### D7. OAuth2 implicit flow (oauth_implicit_default)

This auth uses the OAuth2 Implicit flow, where the access token is returned directly in the redirect.

Steps:

1. In the RQ Request Explorer, select the `oauth_implicit_default_env` environment.
2. Locate the `auth_basic` request under `auth/basic.rq`.
3. Run the `auth_basic` request.
4. When the browser opens, complete the login and consent flow.
5. Allow the browser to redirect back so the extension can capture the token.
6. After the request completes, inspect the **Request headers** for the executed request.

Expected:

- The extension opens a browser and performs the OAuth2 Implicit flow against your provider.
- After successful login, the request to `http://localhost:8080/test` is executed.
- The **Authorization** header is present with a `Bearer` access token obtained from the implicit flow.

### D8. Get OAuth2 access token command (oauth_ac_default)

This scenario validates the `RQ: Get OAuth2 Access Token` command using the same `oauth_ac_default` configuration.

Steps:

1. In the RQ Request Explorer, select the `oauth_ac_default_env` environment.
2. Open the VS Code Command Palette and run `RQ: Get OAuth2 Access Token`.
3. When prompted, select the `oauth_ac_default_env` environment.
4. When prompted, select the `oauth_ac_default` auth configuration.
5. When the browser opens, complete the login and consent flow in your OAuth provider.
6. When prompted, allow the browser to open VS Code so the extension can capture the callback.
7. In VS Code, choose `Copy Token` when the success notification appears.

Expected:

- The command discovers the `oauth_ac_default` auth configuration for the selected environment.
- The browser opens and the full OAuth2 Authorization Code flow completes successfully.
- The access token is copied to the clipboard.
- The token is cached internally so subsequent calls can reuse it.

### D9. Clear OAuth2 access tokens command

This scenario validates the interaction between the `RQ: Get OAuth2 Access Token` command and the `RQ: Clear OAuth2 Access Tokens` command.

Steps:

1. With the `oauth_ac_default_env` environment selected, run `RQ: Get OAuth2 Access Token` and complete the flow once so that a token is cached.
2. Confirm that the command completes without opening the browser again on a second run (token is served from cache).
3. Open the VS Code Command Palette and run `RQ: Clear OAuth2 Access Tokens`.
4. Run `RQ: Get OAuth2 Access Token` again with `oauth_ac_default_env` and `oauth_ac_default`.
5. Complete the browser login and consent flow once more.

Expected:

- After the first successful run, a subsequent `Get OAuth2 Access Token` call reuses the cached token and does not open the browser.
- After running `Clear OAuth2 Access Tokens`, the cache is cleared.
- The next `Get OAuth2 Access Token` call triggers the full browser-based OAuth2 Authorization Code flow again.
- The new access token is obtained and cached successfully.


