import { URLSearchParams, URL } from 'url';
import * as crypto from 'crypto';
import * as vscode from 'vscode';
import { OAuth2Config, OAuth2Result, IOAuth2Flow } from './types';
import { handleVSCodeUriImplicitCallback, ImplicitCallbackResult } from './callbackHandlers';

export class ImplicitFlow implements IOAuth2Flow {
    async execute(config: OAuth2Config): Promise<OAuth2Result> {
        try {
            console.log('Starting OAuth2 Implicit flow');
            console.log('Configuration:', {
                clientId: config.clientId,
                authorizationUrl: config.authorizationUrl,
                redirectUri: config.redirectUri,
                scope: config.scope
            });

            // Validate URLs
            try {
                new URL(config.authorizationUrl);
            } catch (error) {
                console.error('Invalid authorization URL:', config.authorizationUrl);
                throw new Error(`Invalid authorization URL: "${config.authorizationUrl}"`);
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
                response_type: 'token',
            });

            if (config.scope) {
                authParams.set('scope', config.scope);
            }

            if (state) {
                authParams.set('state', state);
            }

            const fullAuthUrl = `${config.authorizationUrl}?${authParams.toString()}`;
            console.log('Authorization URL built:', fullAuthUrl);

            // Get access token directly from callback
            const result = await this.getAccessToken(fullAuthUrl, config.redirectUri);
            console.log('Access token received');

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

            return {
                accessToken: result.accessToken,
                tokenType: result.tokenType,
                expiresIn: result.expiresIn,
                // Implicit flow does not support refresh tokens
            };
        } catch (error) {
            const errorMessage = error instanceof Error ? error.message : String(error);
            console.error('OAuth2 Implicit flow failed:', errorMessage);
            console.error('Full error:', error);
            throw error;
        }
    }

    private async getAccessToken(authUrl: string, redirectUri: string): Promise<ImplicitCallbackResult> {
        const redirectUriLower = redirectUri.toLowerCase();
        
        // Detect redirect URI type and use appropriate handler
        if (redirectUriLower.startsWith('vscode://') || redirectUriLower.startsWith('vscode-insiders://')) {
            console.log('Using VS Code URI handler for callback');
            return handleVSCodeUriImplicitCallback(authUrl);
        } else {
            // Implicit flow usually requires a user-agent to handle the redirect fragment.
            // Using manual paste for implicit flow is difficult unless we ask user to paste the full redirected URL
            // because the fragment is hidden from server logs usually.
            // For now, only support VS Code handler or throw error.
            
            // To be consistent with other flows, we could try to implement manual paste if needed,
            // but the user would have to manually copy the URL from browser address bar which contains #access_token=...
            
            console.log('Implicit flow requires VS Code URI handler currently.');
            
            // We can implement manual paste support
            const callbackUrl = await vscode.window.showInputBox({
                prompt: 'Open the URL in browser, authorize, and paste the FULL result URL here (including #access_token=...)',
                ignoreFocusOut: true
            });

            if (!callbackUrl) {
                throw new Error('Authentication cancelled');
            }

            try {
                // If user pastes full URL
                const url = new URL(callbackUrl);
                let params = new URLSearchParams(url.hash.substring(1)); // remove #
                if (!params.has('access_token')) {
                    params = url.searchParams;
                }
                
                const accessToken = params.get('access_token');
                
                if (accessToken) {
                    return {
                        accessToken,
                        tokenType: params.get('token_type') || 'Bearer',
                        expiresIn: params.get('expires_in') ? parseInt(params.get('expires_in')!, 10) : undefined,
                        state: params.get('state')
                    };
                }
                throw new Error('No access_token found in pasted URL');
            } catch (error) {
                if (error instanceof Error && error.message.startsWith('No access_token')) {
                    throw error;
                }
                // If it's not a valid URL, maybe they just pasted the token?
                // Minimal heuristic: if it looks like a JWT or simple token (long string, no spaces)
                if (callbackUrl.length > 20 && !callbackUrl.includes(' ')) {
                    return {
                        accessToken: callbackUrl,
                        tokenType: 'Bearer',
                        state: null
                    };
                }
                throw new Error('Invalid URL or Token pasted');
            }
        }
    }
}
