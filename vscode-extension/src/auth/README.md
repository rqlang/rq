# OAuth Authentication Module

This module provides OAuth2 authentication capabilities for the RQ VS Code extension.

## Architecture

The OAuth implementation follows a modular, pluggable architecture that makes it easy to add new OAuth2 flows.

### File Structure

```
auth/
├── index.ts                      # Main exports
├── types.ts                      # Type definitions
├── pkceUtils.ts                  # PKCE utility functions
├── callbackHandlers.ts           # Authorization code retrieval strategies
├── authorizationCodePKCEFlow.ts  # OAuth2 Authorization Code with PKCE flow
├── oauthProvider.ts              # Main OAuth provider (VS Code API)
└── README.md                     # This file
```

## Core Components

### 1. Types (`types.ts`)

Defines the core interfaces and types:

- **`OAuth2Config`**: Configuration for OAuth2 flows (clientId, authorizationUrl, tokenUrl, etc.)
- **`OAuth2Result`**: Result from OAuth2 flow execution (accessToken, refreshToken, etc.)
- **`IOAuth2Flow`**: Base interface that all OAuth2 flow implementations must implement
- **`TokenResponse`**: Token endpoint response structure

### 2. PKCE Utils (`pkceUtils.ts`)

Utility functions for PKCE (Proof Key for Code Exchange):

- **`generateCodeVerifier()`**: Generates a cryptographically random code verifier
- **`generateCodeChallenge(verifier, method)`**: Creates a code challenge from verifier using S256 or plain method

### 3. Callback Handlers (`callbackHandlers.ts`)

Multiple strategies for receiving OAuth2 authorization codes:

- **`handleVSCodeUriCallback()`**: Uses VS Code's URI handler for `vscode://` redirect URIs
- **`handleLocalServerCallback()`**: Starts a local HTTP server for `localhost` redirect URIs
- **`handleManualPasteCallback()`**: Prompts user to paste callback URL for external redirect URIs

### 4. Authorization Code PKCE Flow (`authorizationCodePKCEFlow.ts`)

Complete implementation of OAuth2 Authorization Code with PKCE:

- Implements `IOAuth2Flow` interface
- Generates PKCE parameters
- Builds authorization URL
- Retrieves authorization code (auto-detects callback method)
- Exchanges code for access token

### 5. OAuth Provider (`oauthProvider.ts`)

Main provider implementing VS Code's `AuthenticationProvider` API:

- Manages authentication sessions
- Provides `executeOAuth2Flow()` method for programmatic flow execution
- Persists sessions in VS Code's secure storage
- Supports multiple OAuth2 flows through pluggable architecture

## Usage

### Basic Usage

```typescript
import { OAuthProvider, OAuth2Config } from './auth';

// Create provider
const provider = new OAuthProvider(context);

// Register with VS Code
vscode.authentication.registerAuthenticationProvider(
    'my-oauth-provider',
    'My OAuth Provider',
    provider
);
```

### Execute OAuth2 Flow Programmatically

```typescript
import { OAuthProvider, OAuth2Config } from './auth';

const provider = new OAuthProvider(context);

const config: OAuth2Config = {
    clientId: 'my-client-id',
    authorizationUrl: 'https://oauth.example.com/authorize',
    tokenUrl: 'https://oauth.example.com/token',
    redirectUri: 'vscode://publisher.extension/callback',
    scope: 'read write',
    codeChallengeMethod: 'S256'
};

const result = await provider.executeOAuth2Flow(config, 'authorization_code_pkce');
console.log('Access token:', result.accessToken);
```

### Use Individual Components

```typescript
import { AuthorizationCodePKCEFlow, OAuth2Config } from './auth';

const flow = new AuthorizationCodePKCEFlow();
const config: OAuth2Config = { /* ... */ };
const result = await flow.execute(config);
```

## Adding New OAuth2 Flows

To add a new OAuth2 flow (e.g., Implicit, Client Credentials, Device Code):

### 1. Create a new flow file

```typescript
// auth/implicitFlow.ts
import { IOAuth2Flow, OAuth2Config, OAuth2Result } from './types';

export class ImplicitFlow implements IOAuth2Flow {
    async execute(config: OAuth2Config): Promise<OAuth2Result> {
        // Implement implicit flow logic
        // ...
        return {
            accessToken: 'token',
            tokenType: 'Bearer'
        };
    }
}
```

### 2. Register in OAuthProvider

```typescript
// auth/oauthProvider.ts
import { ImplicitFlow } from './implicitFlow';

export class OAuthProvider {
    private readonly implicitFlow: IOAuth2Flow;
    
    constructor(context: vscode.ExtensionContext) {
        this.implicitFlow = new ImplicitFlow();
        // ...
    }
    
    async executeOAuth2Flow(config: OAuth2Config, flowType: string) {
        switch (flowType) {
            case 'implicit':
                return await this.implicitFlow.execute(config);
            // ...
        }
    }
}
```

### 3. Export from index.ts

```typescript
// auth/index.ts
export { ImplicitFlow } from './implicitFlow';
```

## Supported Flows

Currently supported OAuth2 flows:

- ✅ **Authorization Code with PKCE** (`authorization_code_pkce`)
  - Recommended for public clients (like VS Code extensions)
  - Implements RFC 6749 + RFC 7636
  - Supports multiple callback strategies (URI handler, localhost server, manual paste)

Planned flows:

- ⏳ **Client Credentials** - For service-to-service authentication
- ⏳ **Device Code** - For devices with limited input capabilities
- ⏳ **Refresh Token** - For token refresh

## Security Considerations

- **PKCE is mandatory**: All authorization code flows use PKCE (S256 preferred, plain as fallback)
- **Secure storage**: Sessions are stored in VS Code's secure secret storage
- **Origin headers**: Automatically added for CORS compliance with external redirect URIs
- **Timeout protection**: All flows have 5-minute timeout to prevent hanging
- **Error handling**: Comprehensive error messages for debugging

## References

- [RFC 6749 - OAuth 2.0 Authorization Framework](https://tools.ietf.org/html/rfc6749)
- [RFC 7636 - PKCE](https://tools.ietf.org/html/rfc7636)
- [VS Code Authentication Provider API](https://code.visualstudio.com/api/references/vscode-api#AuthenticationProvider)
- [OAuth 2.0 Security Best Practices](https://tools.ietf.org/html/draft-ietf-oauth-security-topics)
