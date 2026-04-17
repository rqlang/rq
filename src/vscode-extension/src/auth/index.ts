/**
 * index.ts # noqa: E501
 * Main exports for OAuth authentication module
 * 
 * This file provides a single entry point for all OAuth-related
 * functionality, making imports cleaner throughout the extension.
 * 
 * Example:
 * ```typescript
 * import { OAuthProvider, OAuth2Config, AuthorizationCodePKCEFlow } from './auth';
 * ```
 */

// Main OAuth provider
export { OAuthProvider } from './oauthProvider';

// OAuth2 flow implementations
export { AuthorizationCodePKCEFlow } from './authorizationCodePKCEFlow';

// Types and interfaces
export type { 
    OAuth2Config, 
    OAuth2Result, 
    IOAuth2Flow,
    TokenResponse 
} from './types';

// Utility functions
export { generateCodeVerifier, generateCodeChallenge } from './pkceUtils';
export * from './oauthService';
export * from './refreshTokenFlow';

// Callback handlers (exported for testing or custom implementations)
export { 
    handleVSCodeUriCallback, 
    handleLocalServerCallback, 
    handleManualPasteCallback 
} from './callbackHandlers';
