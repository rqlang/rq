---
title: Auth
canonical: https://www.rqlang.com/docs/AUTH.html
description: How authentication works in rq and how to configure it for your provider.
---

# Auth

Authentication in rq is configured through **auth providers** — top-level blocks that describe how to obtain credentials for a request.

```
auth my_api(auth_type.bearer) {
  token: api_token,
}

[auth("my_api")]
rq list("https://api.example.com/v1/items");
```

Field values resolve through the normal variable chain, so secrets stay out of `.rq` files and live in `.env` or environment variables instead. See [Secrets](LANGUAGE_DEFINITION.md#secrets) for the full resolution order.

For the formal syntax of `auth` blocks, see the [Language Definition — Auth](LANGUAGE_DEFINITION.md#auth).

## Choosing an auth type

| Situation | Auth type |
| --- | --- |
| The provider gave you a long-lived token (PAT, API key, static JWT) | `bearer` |
| Service-to-service: client ID and secret or certificate, no human involved | `oauth2_client_credentials` |
| You need to sign in as a real user from your local machine | `oauth2_authorization_code` |
| Legacy SPA-style flow, no client secret | `oauth2_implicit` (avoid for new setups) |

- **`bearer`** — "I already have a token."
- **`oauth2_client_credentials`** — "Give me a token using these client credentials (secret or certificate)."
- **`oauth2_authorization_code`** — "Open a browser, let the user sign in, then give me a token."

## Redirect URIs for interactive flows

`oauth2_authorization_code` and `oauth2_implicit` flows require user interaction — a browser opens for sign-in and control must be passed back to rq after login. **These flows are only supported in the VS Code extension.** The CLI has no browser integration and cannot run them. To use an interactive auth config from the CLI, inject a precomputed token instead — see [Injecting a precomputed token](#injecting-a-precomputed-token).

When running from VS Code, the redirect handling strategy is determined by the `redirect_uri` you set in your `auth` block.

### VS Code URI handler (default)

If you do not set `redirect_uri`, rq uses:

```
vscode://rq-lang.rq-language/oauth-callback
```

After sign-in the browser redirects to this URI and VS Code catches the callback automatically — no manual copy-pasting required. This is the recommended option and works across local setups, WSL, SSH remotes, and Codespaces.

Register this exact URI in your provider's app settings. It is case-sensitive.

### Localhost server

If you set `redirect_uri` to a `http://localhost:...` URL, the extension temporarily starts a local HTTP server on that port, captures the authorization code after sign-in, then stops the server:

```
auth my_api(auth_type.oauth2_authorization_code) {
  ...
  redirect_uri: "http://localhost:3000/callback",
}
```

Use this when your provider supports localhost redirects but not custom URI schemes (`vscode://`).

Register the localhost URI in your provider's app settings.

### Manual (custom URL)

For any other `redirect_uri` value — such as a production callback URL — the extension opens a browser, waits for you to complete the sign-in flow, then shows an input box asking for the full redirect URL. Copy it from the browser's address bar and paste it in; the extension extracts the authorization code from the URL.

```
auth my_api(auth_type.oauth2_authorization_code) {
  ...
  redirect_uri: "https://my-api.com/auth/callback",
}
```

Register the URL in your provider's app settings.

## Runtime overrides

### Injecting a precomputed token

For `oauth2_authorization_code` and `oauth2_implicit` flows, the reserved variable `auth_token` short-circuits the interactive browser step:

```
rq request run -n list_widgets -v auth_token=eyJhbGciOi...
```

When `auth_token` is supplied, rq sends it as `Authorization: Bearer <auth_token>` and skips the browser step. This lets the same `.rq` file work interactively from VS Code and headlessly from CI pipelines that mint their own tokens.

## AI-assisted setup

Use these prompts with an AI assistant to gather the values you need before writing your `auth` block.

### Finding endpoint URLs

Use this prompt to locate the `authorization_endpoint` and `token_endpoint` for your OAuth2 provider without guessing:

```
I'm configuring OAuth2 and I need the authorization and token endpoint URLs for my identity provider.
I will look them up myself — your job is to tell me exactly where.

Provider: [your provider, e.g. Azure Entra ID, Okta, Auth0, Keycloak…]
My setup (fill any that apply):
  tenant/org/domain: [...]

Do two things:
1. Give me the full URL of this provider's OpenID Connect discovery document
   (the .well-known/openid-configuration path), built for my setup above.
   I will open it and read "authorization_endpoint" and "token_endpoint" from
   the JSON myself.
2. As a fallback, tell me where in the provider's admin portal these two URLs
   are shown — the exact menu path to click — in case I can't reach the
   discovery document.

Do not guess the endpoint URLs themselves. I only want the discovery document
URL and the navigation path.
```

### Configuring Authorization Code (PKCE)

Use this prompt to locate every required field for an `oauth2_authorization_code` block, including provider quirks:

```
I'm configuring an OAuth2 Authorization Code flow (with PKCE) and I need to
locate the required configuration values in my identity provider's admin portal.
I will look them up myself — your job is to tell me exactly where.

Provider: [your provider, e.g. Azure Entra ID, Okta, Auth0, Keycloak…]
My setup (fill any that apply):
  tenant/org/domain: [...]
  app type (web / SPA / native-public): [...]

For this provider, do the following. Do not guess any actual values (no real
client IDs, secrets, or URLs unless they are fixed and provider-wide) — give
me locations and formats only.

1. Field-by-field navigation. For each of these Authorization Code fields,
   give the exact menu path to click in the provider's current admin portal
   to find or set it:
     - client_id
     - client_secret (and whether this provider supports/encourages public
       clients without one)
     - redirect_uri (where to register it, and any format constraints)
     - scopes
   Use the provider's real menu labels.

2. Provider-specific quirks. Call out anything non-obvious about this
   provider's Authorization Code setup, especially:
     - Scope format — does this provider require a special scope syntax?
       (e.g. Azure Entra ID requires api://{client_id}/.default for
       app-permission-style access, Google requires full scope URLs, etc.)
     - Whether a separate API/resource registration, audience, or consent
       step is required before scopes work.
     - Redirect URI rules (exact match, no wildcards, loopback/localhost
       handling for native apps).
     - PKCE: required, optional, or enforced for certain app types.
     - Any field that is named differently than the OAuth2 spec term
       (e.g. "Application (client) ID").
```

### Configuring Client Credentials

Use this prompt to locate every required field for an `oauth2_client_credentials` block, including permission grant models and provider quirks:

```
I'm configuring an OAuth2 Client Credentials flow (machine-to-machine, no
user sign-in) and I need to locate the required configuration values in my
identity provider's admin portal. I will look them up myself — your job is
to tell me exactly where.

Provider: [your provider, e.g. Azure Entra ID, Okta, Auth0, Keycloak…]
My setup (fill any that apply):
  tenant/org/domain: [...]
  the API/resource this client needs to call: [...]

For this provider, do the following. Do not guess any actual values (no real
client IDs, secrets, or URLs unless they are fixed and provider-wide) — give
me locations and formats only.

1. Field-by-field navigation. For each of these Client Credentials fields,
   give the exact menu path to click in the provider's current admin portal
   to find or set it:
     - client_id
     - client_secret (where to generate one, since it can't be retrieved
       after creation — and whether certificate/private-key-JWT auth is
       offered or required instead)
     - scopes/audience
   Use the provider's real menu labels.

2. Provider-specific quirks. Call out anything non-obvious about this
   provider's Client Credentials setup, especially:
     - Scope/audience format — what does this provider expect?
       (e.g. Azure Entra ID requires api://{resource_client_id}/.default or
       https://graph.microsoft.com/.default; some providers use an audience
       parameter instead of scope; Auth0 requires the API Identifier as
       audience.)
     - Permission grant model — for Client Credentials there is no
       interactive user consent, so how are permissions authorized?
       (e.g. Azure "Application permissions" + admin consent; Google service
       accounts + domain-wide delegation; provider-side API
       authorization/grant.)
     - Whether the client must be a specific app type (confidential client
       only — no public clients), and whether a separate API/resource must
       be registered or expose itself first.
     - Client secret constraints: expiry/rotation, max lifetime, whether
       the portal shows it only once.
     - Any field named differently than the OAuth2 spec term.
```

## Troubleshooting

- Verify the redirect URI is registered byte-for-byte — `vscode://rq-lang.rq-language/oauth-callback` is case-sensitive.
- Check that the scope format matches the provider: Azure wants `api://{id}/.default`, Google wants full URLs, AWS Cognito wants `resource-server/scope`.
- Enable debug logging to see the full token exchange and the headers actually sent. In the CLI, pass `-d` / `--debug`. In the VS Code extension, enable `rq.debugLogging` in Settings.
- **Check the redirect URL for error details.** When auth is misconfigured, many providers encode the error in the redirect URL as query parameters (e.g. `?error=unauthorized_client&error_description=...`). If the sign-in flow fails silently or lands on an unexpected page, inspect the full URL in the browser's address bar — the error description is often right there.
- **Confirm the auth type matches the provider's configuration.** `oauth2_authorization_code` requires the authorization code flow to be enabled on the provider side, and `oauth2_client_credentials` requires a client secret and the client credentials grant to be enabled. Using the wrong flow type is a common source of `unauthorized_client` errors.

## See also

- [Language Definition — Auth](LANGUAGE_DEFINITION.md#auth) — formal syntax and the full list of supported fields.
- [Language Definition — Secrets](LANGUAGE_DEFINITION.md#secrets) — how `.env` files and OS variables are resolved.
- [CLI — `rq auth`](CLI.md#managing-auth-providers-rq-auth) — inspecting auth providers from the command line.
- [VS Code Extension](VSCODE_EXTENSION.md) — interactive sign-in and how the redirect URI is handled.
