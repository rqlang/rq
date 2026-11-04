import { URLSearchParams, URL } from 'url';
import * as crypto from 'crypto';
import { AuthUi, OAuth2Config, OAuth2Result, IOAuth2Flow } from './types';
import { verifyState } from './signInErrors';
import { handleAppRedirectImplicitCallback, handleManualPasteImplicitCallback, isAppRedirect, ImplicitCallbackResult } from './callbackHandlers';

export class ImplicitFlow implements IOAuth2Flow {
    constructor(private readonly ui: AuthUi) {}

    async execute(config: OAuth2Config): Promise<OAuth2Result> {
        try {
            this.ui.log(`Starting OAuth2 Implicit flow for client ${config.clientId}`);
            try {
                new URL(config.authorizationUrl);
            } catch {
                throw new Error(`Invalid authorization URL: "${config.authorizationUrl}"`);
            }

            const state = config.useState !== false ? crypto.randomBytes(32).toString('hex') : undefined;
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

            const result = await this.getAccessToken(`${config.authorizationUrl}?${authParams.toString()}`, config.redirectUri);
            if (config.useState !== false) {
                verifyState(state, result.state);
            }

            return {
                accessToken: result.accessToken,
                tokenType: result.tokenType,
                expiresIn: result.expiresIn,
            };
        } catch (error) {
            this.ui.log(`OAuth2 Implicit flow failed: ${error instanceof Error ? error.message : String(error)}`);
            throw error;
        }
    }

    private async getAccessToken(authUrl: string, redirectUri: string): Promise<ImplicitCallbackResult> {
        if (isAppRedirect(redirectUri)) {
            return handleAppRedirectImplicitCallback(this.ui, authUrl, redirectUri);
        }
        return handleManualPasteImplicitCallback(this.ui, authUrl);
    }
}
