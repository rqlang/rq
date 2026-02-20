import { URLSearchParams, URL } from 'url';
import * as crypto from 'crypto';
import { OAuth2Config, OAuth2Result, IOAuth2Flow, TokenResponse } from './types';
import { generateCodeVerifier, generateCodeChallenge } from './pkceUtils';
import { handleVSCodeUriCallback, handleLocalServerCallback, handleManualPasteCallback, CallbackResult } from './callbackHandlers';

export class AuthorizationCodePKCEFlow implements IOAuth2Flow {
    async execute(config: OAuth2Config): Promise<OAuth2Result> {
        try {
            console.log('Starting OAuth2 Authorization Code with PKCE flow');
            console.log('Configuration:', {
                clientId: config.clientId,
                authorizationUrl: config.authorizationUrl,
                tokenUrl: config.tokenUrl,
                redirectUri: config.redirectUri,
                scope: config.scope,
                codeChallengeMethod: config.codeChallengeMethod
            });

            // Validate URLs
            try {
                new URL(config.authorizationUrl);
            } catch (error) {
                console.error('Invalid authorization URL:', config.authorizationUrl);
                throw new Error(`Invalid authorization URL: "${config.authorizationUrl}"`);
            }

            try {
                if (!config.tokenUrl) {
                    throw new Error('Token URL is required for Authorization Code flow');
                }
                new URL(config.tokenUrl);
            } catch (error) {
                console.error('Invalid token URL:', config.tokenUrl);
                throw new Error(`Invalid token URL: "${config.tokenUrl}"`);
            }

            // Generate PKCE parameters
            let codeVerifier: string | undefined;
            let challengeMethod: 'S256' | 'plain' | undefined;
            let codeChallenge: string | undefined;

            if (config.usePkce !== false) {
                codeVerifier = generateCodeVerifier();
                challengeMethod = config.codeChallengeMethod || 'S256';
                codeChallenge = generateCodeChallenge(codeVerifier, challengeMethod);
                console.log('PKCE parameters generated');
            } else {
                console.log('PKCE disabled by configuration');
            }

            // Generate state if not disabled (default is enabled)
            let state: string | undefined;
            if (config.useState !== false) {
                state = crypto.randomBytes(32).toString('hex');
                console.log('State parameter generated');
            }

            // Build authorization URL
            const authParams = new URLSearchParams({
                client_id: config.clientId,
                redirect_uri: config.redirectUri,
                response_type: 'code',
            });

            if (codeChallenge && challengeMethod) {
                authParams.set('code_challenge_method', challengeMethod);
                authParams.set('code_challenge', codeChallenge);
            }

            if (config.scope) {
                authParams.set('scope', config.scope);
            }

            if (state) {
                authParams.set('state', state);
            }

            const fullAuthUrl = `${config.authorizationUrl}?${authParams.toString()}`;
            console.log('Authorization URL built:', fullAuthUrl);

            // Get authorization code
            const result = await this.getAuthorizationCode(fullAuthUrl, config.redirectUri);
            console.log('Authorization code received');

            // Verify state
            if (config.useState !== false) {
                if (!result.state) {
                    throw new Error('State parameter missing in callback');
                }
                if (result.state !== state) {
                    throw new Error('State parameter mismatch');
                }
                console.log('State parameter verified');
            }

            // Exchange code for token
            const tokenResponse = await this.exchangeCodeForToken(result.code, codeVerifier, config);
            console.log('Token exchange successful');

            return {
                accessToken: tokenResponse.access_token,
                tokenType: tokenResponse.token_type,
                expiresIn: tokenResponse.expires_in,
                refreshToken: tokenResponse.refresh_token,
                scope: tokenResponse.scope,
            };
        } catch (error) {
            const errorMessage = error instanceof Error ? error.message : String(error);
            console.error('OAuth2 PKCE flow failed:', errorMessage);
            console.error('Full error:', error);
            throw error;
        }
    }

    private async getAuthorizationCode(authUrl: string, redirectUri: string): Promise<CallbackResult> {
        const redirectUriLower = redirectUri.toLowerCase();
        
        // Detect redirect URI type and use appropriate handler
        if (redirectUriLower.startsWith('vscode://') || redirectUriLower.startsWith('vscode-insiders://')) {
            console.log('Using VS Code URI handler for callback');
            return handleVSCodeUriCallback(authUrl);
        } else if (redirectUriLower.startsWith('http://localhost') || redirectUriLower.startsWith('http://127.0.0.1')) {
            console.log('Using local HTTP server for callback');
            return handleLocalServerCallback(authUrl, redirectUri);
        } else {
            console.log('Using manual paste for external redirect URI');
            return handleManualPasteCallback(authUrl, redirectUri);
        }
    }

    private async exchangeCodeForToken(
        code: string,
        codeVerifier: string | undefined,
        config: OAuth2Config
    ): Promise<TokenResponse> {
        console.log('Exchanging authorization code for token...');
        
        if (!config.tokenUrl) {
            throw new Error('Token URL is required for Authorization Code flow');
        }

        const params: Record<string, string> = {
            client_id: config.clientId,
            grant_type: 'authorization_code',
            code: code,
            redirect_uri: config.redirectUri,
        };

        if (codeVerifier) {
            params.code_verifier = codeVerifier;
        }

        if (config.clientSecret) {
            params.client_secret = config.clientSecret;
        }

        const headers: Record<string, string> = {
            'Content-Type': 'application/x-www-form-urlencoded'
        };

        // Add Origin header for external redirect URIs
        const redirectUriLower = config.redirectUri.toLowerCase();
        const isExternal = !redirectUriLower.startsWith('http://localhost') && 
                          !redirectUriLower.startsWith('http://127.0.0.1') &&
                          !redirectUriLower.startsWith('vscode://') &&
                          !redirectUriLower.startsWith('vscode-insiders://');
        
        if (isExternal) {
            try {
                const redirectUrl = new URL(config.redirectUri);
                const origin = `${redirectUrl.protocol}//${redirectUrl.host}`;
                headers['Origin'] = origin;
                console.log('Adding Origin header:', origin);
            } catch (error) {
                console.warn('Failed to parse redirect URI for Origin header:', error);
            }
        }

        const response = await fetch(config.tokenUrl, {
            method: 'POST',
            headers: headers,
            body: new URLSearchParams(params).toString()
        });

        if (!response.ok) {
            const errorText = await response.text();
            console.error('Token exchange failed:', response.status, errorText);
            throw new Error(`Token exchange failed: ${response.status} ${response.statusText}\n${errorText}`);
        }

        const tokenResponse = await response.json() as TokenResponse;
        console.log('Token exchange successful');
        return tokenResponse;
    }
}
