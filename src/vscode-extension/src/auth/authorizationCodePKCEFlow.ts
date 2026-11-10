import { URLSearchParams, URL } from 'url';
import * as crypto from 'crypto';
import { AuthUi, OAuth2Config, OAuth2Result, IOAuth2Flow, TokenResponse } from './types';
import { generateCodeVerifier, generateCodeChallenge } from './pkceUtils';
import { tokenExchangeError, verifyState } from './signInErrors';
import { handleAppRedirectCallback, handleLocalServerCallback, handleManualPasteCallback, isAppRedirect, isLoopbackRedirect, CallbackResult } from './callbackHandlers';

export class AuthorizationCodePKCEFlow implements IOAuth2Flow {
    constructor(private readonly ui: AuthUi) {}

    async execute(config: OAuth2Config): Promise<OAuth2Result> {
        try {
            this.ui.log(`Starting OAuth2 Authorization Code with PKCE flow for client ${config.clientId}`);
            validateUrls(config);

            const codeVerifier = generateCodeVerifier();
            const challengeMethod = config.codeChallengeMethod || 'S256';
            const codeChallenge = generateCodeChallenge(codeVerifier, challengeMethod);

            const state = config.useState !== false ? crypto.randomBytes(32).toString('hex') : undefined;
            const authParams = new URLSearchParams({
                client_id: config.clientId,
                redirect_uri: config.redirectUri,
                response_type: 'code',
                code_challenge_method: challengeMethod,
                code_challenge: codeChallenge,
            });
            if (config.scope) {
                authParams.set('scope', config.scope);
            }
            if (state) {
                authParams.set('state', state);
            }

            const result = await this.getAuthorizationCode(`${config.authorizationUrl}?${authParams.toString()}`, config.redirectUri);
            if (config.useState !== false) {
                verifyState(state, result.state);
            }

            const tokenResponse = await this.exchangeCodeForToken(result.code, codeVerifier, config);
            return {
                accessToken: tokenResponse.access_token,
                tokenType: tokenResponse.token_type,
                expiresIn: tokenResponse.expires_in,
                refreshToken: tokenResponse.refresh_token,
                scope: tokenResponse.scope,
            };
        } catch (error) {
            this.ui.log(`OAuth2 PKCE flow failed: ${error instanceof Error ? error.message : String(error)}`);
            throw error;
        }
    }

    private async getAuthorizationCode(authUrl: string, redirectUri: string): Promise<CallbackResult> {
        if (isAppRedirect(redirectUri)) {
            return handleAppRedirectCallback(this.ui, authUrl, redirectUri);
        }
        if (isLoopbackRedirect(redirectUri)) {
            return handleLocalServerCallback(this.ui, authUrl, redirectUri);
        }
        return handleManualPasteCallback(this.ui, authUrl, redirectUri);
    }

    private async exchangeCodeForToken(code: string, codeVerifier: string, config: OAuth2Config): Promise<TokenResponse> {
        if (!config.tokenUrl) {
            throw new Error('Token URL is required for Authorization Code flow');
        }
        const params: Record<string, string> = {
            client_id: config.clientId,
            grant_type: 'authorization_code',
            code,
            redirect_uri: config.redirectUri,
            code_verifier: codeVerifier,
        };
        if (config.clientSecret) {
            params.client_secret = config.clientSecret;
        }
        const headers: Record<string, string> = { 'Content-Type': 'application/x-www-form-urlencoded' };
        if (!isAppRedirect(config.redirectUri) && !isLoopbackRedirect(config.redirectUri)) {
            try {
                const redirectUrl = new URL(config.redirectUri);
                headers['Origin'] = `${redirectUrl.protocol}//${redirectUrl.host}`;
            } catch {
                this.ui.log('Failed to parse redirect URI for Origin header');
            }
        }

        const response = await fetch(config.tokenUrl, {
            method: 'POST',
            headers,
            body: new URLSearchParams(params).toString(),
            signal: this.ui.signal
        });
        if (!response.ok) {
            const errorText = await response.text();
            throw tokenExchangeError(config.tokenUrl, response.status, errorDetail(errorText));
        }
        this.ui.log('Token exchange successful');
        return await response.json() as TokenResponse;
    }
}

export function errorDetail(errorText: string): string {
    try {
        const parsed = JSON.parse(errorText);
        return parsed.error_description || parsed.error_message || parsed.error || parsed.message || errorText;
    } catch {
        return errorText;
    }
}

function validateUrls(config: OAuth2Config): void {
    try {
        new URL(config.authorizationUrl);
    } catch {
        throw new Error(`Invalid authorization URL: "${config.authorizationUrl}"`);
    }
    try {
        if (!config.tokenUrl) {
            throw new Error('Token URL is required for Authorization Code flow');
        }
        new URL(config.tokenUrl);
    } catch {
        throw new Error(`Invalid token URL: "${config.tokenUrl}"`);
    }
}
