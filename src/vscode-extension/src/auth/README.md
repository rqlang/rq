# OAuth Authentication Module

Interactive OAuth2 token acquisition for the flows rq-lib cannot run on its own: authorization code (with or without PKCE) and implicit. rq-lib prepares the request and reports `token_required`; this module obtains the token, and the caller feeds it back as the `auth_token` variable.

The module has no dependency on VS Code. Everything that touches a user interface or a store goes through two interfaces in `types.ts`, so the extension and the MCP server share the same flows:

- **`AuthUi`**: open a URL, ask for a pasted redirect URL, wait for an app redirect (optional), show an error, log.
- **`TokenStore`**: read, write and clear the serialized token cache.

## Files

```
auth/
├── types.ts                      # OAuth2Config, OAuth2Result, AuthUi, TokenStore, OAuthAuthConfig
├── oauthService.ts               # performOAuth2Flow: auth fields from rq-lib → OAuth2Config → token
├── oauthProvider.ts              # Token cache per auth name and environment, refresh, flow dispatch
├── authorizationCodePKCEFlow.ts  # Authorization code flow, PKCE and state, code exchange
├── implicitFlow.ts               # Implicit flow
├── refreshTokenFlow.ts           # Refresh token grant
├── callbackHandlers.ts           # How the authorization response comes back (see below)
├── pkceUtils.ts                  # Code verifier and challenge
├── vscodeAuth.ts                 # VS Code adapter: AuthUi, SecretStorage TokenStore, vscodeOAuthProvider
└── authUriHandler.ts             # VS Code URI handler behind waitForAppRedirect
```

## How the authorization response comes back

Chosen from `redirect_uri`:

| `redirect_uri` | Handler | Needs |
|---|---|---|
| `vscode://…` (the default) | `handleAppRedirectCallback` | `AuthUi.waitForAppRedirect`; without it the flow fails before opening anything |
| `http://localhost:N`, `http://127.0.0.1:N` | `handleLocalServerCallback` | Nothing: a local server on that port takes one response |
| Anything else | `handleManualPasteCallback` | `AuthUi.askForRedirectUrl` |

The implicit flow uses the app redirect for `vscode://` and the pasted URL otherwise. Every handler times out after five minutes.

## Usage

In the extension:

```typescript
import { performOAuth2Flow } from './auth';
import { vscodeOAuthProvider } from './auth/vscodeAuth';

const token = await performOAuth2Flow(preparedAuth, vscodeOAuthProvider(context, outputChannel));
```

Another host builds its own `OAuthProvider` from an `AuthUi` and a `TokenStore`. One provider instance keeps the cache it loaded, so a host that keeps the instance alive avoids rereading the store.

## Security

- PKCE (S256 by default) and `state` are on unless the auth sets `use_pkce: false` or `use_state: false`.
- The extension keeps tokens in VS Code's secret storage under `oauth-token-cache`.
- The local callback page escapes what the provider sends back and closes the connection after one response.
