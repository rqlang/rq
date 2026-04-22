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

## A. Diagnostics

### A1. Syntax error

Steps:

1. Open [errors/syntax_error.rq](errors/syntax_error.rq).

Expected:

- The offending line is underlined in red immediately.
- The Problems panel shows one error from source `rq` with the correct line and column.

### A2. Validation error — wrong auth property

Steps:

1. Open [errors/validation_error.rq](errors/validation_error.rq).

Expected:

- An error is reported on the `token_url:` line — `bearer` auth does not accept that property.
- The Problems panel shows one error pointing to that line.

### A3. Missing variable

Steps:

1. Open [errors/missing_variable.rq](errors/missing_variable.rq).

Expected:

- Two errors appear, one for each undefined variable (`base_url`, `user_id`).
- Each underline points to the exact `{{variable}}` reference inside the URL string.

### A4. Duplicate request name

Steps:

1. Open [errors/duplicate_name.rq](errors/duplicate_name.rq).

Expected:

- An error is reported on the second `rq get_users` declaration.
- The first declaration has no error.

### A5. Real-time diagnostics — error appears while typing

Steps:

1. Create a new empty `.rq` file inside the `tests/uat` workspace (e.g. `scratch.rq`).
2. Type a valid request: `rq ping("http://localhost:8080/test");`
3. Introduce a syntax error by removing the closing `"` from the URL.

Expected:

- Within ~1 second of stopping typing, a red underline appears on the broken line.
- The Problems panel updates automatically — no save required.

### A6. Real-time diagnostics — error clears on fix

Steps:

1. Continue from B5 with the syntax error still present.
2. Restore the closing `"` to make the line valid again.

Expected:

- The underline disappears within ~1 second.
- The Problems panel clears the entry for that file.

---

## B. Using the Request Explorer to execute requests

### B1. Execute a request without selecting an environment

Steps:

1. Open the `tests/uat` folder in VS Code.
2. Open the RQ Request Explorer view.
3. Locate the `get_basic` request defined in `requests/basic.rq`.
4. Execute the request from the explorer (the “Run Request” action).

Expected:

- The request executes successfully against `http://localhost:8080/test`.
- The “RQ” output channel shows the executed command and the response.
- No new errors appear in the Problems view related to this execution.

### B2. Execute a request with an environment

Steps:

1. With the `tests/uat` folder still open, go to the RQ Request Explorer view.
2. Select the `local` environment in the explorer’s environment selector.
3. Locate the `shared_with_env` request defined in `requests/basic.rq`.
4. Execute the request from the explorer.

Expected:

- The request executes using the selected `local` environment.
- The URL used respects the environment values (for example `base_url` and `api_path`).
- The “RQ” output channel reflects the environment used and the execution completes without errors.

### B3. Execute a request with variables

Steps:

1. With the `tests/uat` folder open in VS Code, go to the RQ Request Explorer view.
2. Locate the `users.get` request under `requests/endpoints.rq`.
3. Run the request once without providing any extra variables.
4. Run the same `users.get` request again, this time providing the variable `user_id=321` when prompted (or via the variables UI).

Expected:

- The first execution uses the default value `user_id = 123` and the URL is `http://localhost:8080/users/123?v=1`.
- The second execution uses the overridden value `user_id = 321` and the URL is `http://localhost:8080/users/321?v=1`.

### B4. Execute a request with required variables

Steps:

1. With the `tests/uat` folder open in VS Code, go to the RQ Request Explorer view.
2. Locate the `users.create` request under `requests/endpoints.rq`.
3. Run the request **without** providing any variables.

Expected:

- The request fails with a validation error: `Required variable(s) not set: user_name, user_role`.
- A VS Code error notification appears and the output channel shows the error.
- No HTTP request is sent.

4. Run `users.create` again, this time providing both required variables — for example `user_name=Alice` and `user_role=admin` — when prompted.

Expected:

- The request executes successfully as a `POST` to `http://localhost:8080/users?v=1`.
- The request body is `{"name": "Alice", "role": "admin"}`.
- Hovering over `user_name` or `user_role` in the editor shows `*(required)* — Must be provided at runtime via --var`.

### B5. Execute a request when the echo server is not running

Steps:

1. Stop the local echo server if it is running (port `8080` must be unavailable).
2. With the `tests/uat` folder open in VS Code, go to the RQ Request Explorer view.
3. Locate the `get_basic` request defined in `requests/basic.rq`.
4. Execute the request from the explorer.

Expected:

- The request fails immediately.
- A VS Code error notification appears indicating the request failed, with a "Show Output" action.
- Opening the "RQ" output channel shows a `Request Failed: get_basic` block with a human-readable message such as `error sending request for url (http://localhost:8080/test): ... Connection refused`.
- The error message is plain text — not raw JSON.
- No crash or unhandled exception occurs in the extension.

---

## C. OAuth authentication flows

The extension supports multiple redirect handling strategies:

- VS Code URI handler: `vscode://rq-lang.rq-language/...`
- Localhost redirect capture: `http://localhost:<port>/...`
- Manual copy/paste of the final redirect URL

These tests assume you have an OAuth provider configured in your `.rq` files and that you use the consolidated auth fixtures.

### C0. Common setup

1. Use the auth fixtures under:
   - [auth/shared.rq](auth/shared.rq)
   - [auth/basic.rq](auth/basic.rq)
2. Copy `.env.example` in this folder to a new file named `.env`

Expected:

- Each environment in shared.rq exposes an `auth_name` value that selects one auth configuration.
- The `.env` file is present and contains valid values so the variables used in shared.rq can be resolved.
- The auth config is discoverable by the extension when running requests from basic.rq.

### C1. Bearer token auth (bearer_auth)

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

### C2. OAuth2 authorization code (default redirect) (oauth_ac_default)

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

### C3. OAuth2 authorization code (custom localhost redirect) (oauth_ac_custom)

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

### C4. OAuth2 authorization code (external redirect) (oauth_ac_external)

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

### C5. OAuth2 client credentials (shared secret) (oauth_cc)

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

### C6. OAuth2 client credentials with client certificate (oauth_cc_cert)

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

### C7. OAuth2 implicit flow (oauth_implicit_default)

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

### C8. Get OAuth2 access token command (oauth_ac_default)

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

### C9. Clear OAuth2 access tokens command

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

---

## D. Autocomplete

All scenarios in this section start from [requests/autocomplete.rq](requests/autocomplete.rq), which is an empty file. Work through the tests in order — each one adds content to the file using autocomplete, so later tests build on what was typed in earlier ones.

### D1. Variables — declare with `let`

Steps:

1. Open `requests/autocomplete.rq`.
2. Type `let base = ` and trigger autocomplete (Ctrl+Space / ⌃Space).

Expected:

- Built-in function snippets appear: `random.guid()`, `datetime.now()`, `io.read_file()`.
- No variables appear yet (file is empty).
- Dismiss the list and finish typing `"http://localhost:8080";` manually.

### D2. Variables — second `let` suggests existing variable

Steps:

1. On a new line type `let user_id = ` and trigger autocomplete.

Expected:

- `base` appears in the list as a variable suggestion.
- Built-in functions are still offered.
- Dismiss and type `"123";` manually.

### D3. Variable interpolation inside a string

Steps:

1. On a new line start typing `rq get_users("{{` and observe the completion list (it triggers automatically after `{{`).

Expected:

- Both `base` and `user_id` appear as completions.
- Selecting `base` inserts the name; the cursor lands after it, ready to type `}}/users");`.
- Complete the line to `rq get_users("{{base}}/users");`.

### D4. `rq` — positional first argument

Steps:

1. On a new line type `rq post_user(` and trigger autocomplete.

Expected:

- The list includes the defined variables (`base`, `user_id`) and built-in functions.
- Named parameter hints (`url:`, `headers:`, `body:`) are also offered.
- Dismiss and continue building the statement in the next tests.

### D5. `rq` — named parameter completion and deduplication

Steps:

1. Select `url:` from the list. The editor inserts `url: ` and positions the cursor at the value.
2. Type `"{{base}}/users",` then press Enter.
3. Trigger autocomplete on the new line.

Expected:

- `url:` is no longer offered.
- Only `headers:` and `body:` remain.
- Select `headers:` to continue. Complete the statement:
  ```
  rq post_user(
      url: "{{base}}/users",
      headers: ["Content-Type": "application/json"],
      body: ${"name": "Alice"},
  );
  ```

### D6. `ep` — named parameter completion

Steps:

1. On a new line type `ep api(` and trigger autocomplete.

Expected:

- The list offers `url:`, `headers:`, and `qs:` as named parameters alongside variables.
- Select `url:`, type `"{{base}}",`, press Enter, trigger again.
- `url:` is gone; `headers:` and `qs:` remain.
- Complete to `ep api(url: "{{base}}", qs: "v=1");`.

### D7. `ep` body — `rq` inside an endpoint

Steps:

1. Add a new endpoint with a body:
   ```
   ep users<
   ```
   Trigger autocomplete after `<`.

Expected:

- `api` appears as a template option (the `ep` declared without a `{}` body in E6).
- Select `api` to get `ep users<api>`.
- Complete the declaration: `ep users<api>("/users") {` and press Enter.
2. Inside the body type `rq ` and trigger autocomplete — no crash, normal identifier completion.
3. Type `rq list();` and close the block with `}`.

### D8. Attribute — `[` at line start

Steps:

1. On a new line before `rq list();` type `[` and trigger autocomplete.

Expected:

- Exactly three completions: `method`, `timeout`, `auth`.
- Verify no completions appear when `[` is inside a header dict (e.g. type `let h = [` on a scratch line — no attribute completions).

### D9. Attribute — method values

Steps:

1. Select `method` from the list in E8.

Expected:

- The snippet `method(GET)` is inserted with `GET` as a tab-stop choice.
- Cycling through choices shows all HTTP verbs: `GET`, `POST`, `PUT`, `DELETE`, `PATCH`, `HEAD`, `OPTIONS`.
- Select `POST` and confirm the line reads `[method(POST)]`.

### D10. `auth` block — auth type completion

Steps:

1. On a new line type `auth my_auth(` and trigger autocomplete.

Expected:

- `auth_type` is suggested. Select it — the editor inserts `auth_type.` and re-triggers completions.
- The list shows exactly four entries: `bearer`, `oauth2_authorization_code`, `oauth2_client_credentials`, `oauth2_implicit`.
- Each has a detail label and documentation.
- Select `bearer` and complete the line: `auth my_auth(auth_type.bearer) {`.

### D11. `auth` block — property completions

Steps:

1. Press Enter to create a blank line inside the block and trigger autocomplete.

Expected:

- `token:` appears as a required property (sorted first).
- Select `token:` — inserts `token: ""` with cursor inside the quotes.
- Type a value (e.g. `"abc"`), press Enter, and trigger autocomplete again.

Expected (second trigger):

- `token:` is no longer offered (already defined).
- No crash or duplicates.
- Close the block with `}`.

### D12. `[auth("` — auth name completion

Steps:

1. On a new line type `[auth("` and trigger autocomplete.

Expected:

- `my_auth` appears in the completion list.
- Selecting it inserts the name between the quotes: `[auth("my_auth")]`.
- Complete the attribute and add a request below it:
  ```
  [auth("my_auth")]
  rq secure("{{base}}/secure");
  ```

### D13. Header key completions inside array literals

Steps:

1. Start a new `rq` with a headers array:
   ```
   rq with_headers(
       "{{base}}/test",
       ["
   ```
   Trigger autocomplete after the opening `"` inside the `[`.

Expected:

- Common HTTP header names appear (`Accept`, `Authorization`, `Content-Type`, `X-Api-Key`, etc.).
- Selecting a header inserts `"Header-Name": ""` with the cursor at the value.
- Typing a partial name (e.g. `"Con`) filters the list to `Content-Length`, `Content-Type`.

### D14. `ep crud` snippet

Steps:

1. On a new line type `ep` and trigger autocomplete.
2. Select the `ep crud` entry from the list.

Expected:

- The snippet expands to a full CRUD endpoint block:
  ```
  let {name}_id = "";

  ep {name}s(<cursor>) {
      rq list();
      rq get();
      rq post(body: io.read_file("{name}-post.json"));
      rq patch(url: {name}_id, body: io.read_file("{name}-patch.json"));
      rq delete();
  }
  ```
- The first tab stop is the entity name (e.g. type `widget`) — all occurrences update simultaneously: `widget_id`, `ep widgets`, `widget-post.json`, `widget-patch.json`.
- The second tab stop (cursor `$0`) lands inside the `ep` parameter list, ready to type the base URL.


