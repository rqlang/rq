import { URLSearchParams } from 'url';
import { OAuth2Config, OAuth2Result, TokenResponse } from './types';
import { errorDetail } from './authorizationCodePKCEFlow';

export class RefreshTokenFlow {
    constructor(private readonly log: (message: string) => void = () => undefined) {}

    async execute(config: OAuth2Config, refreshToken: string): Promise<OAuth2Result> {
        if (!config.tokenUrl) {
            throw new Error('Token URL is required for Refresh Token flow');
        }
        const params = new URLSearchParams({
            grant_type: 'refresh_token',
            client_id: config.clientId,
            refresh_token: refreshToken,
        });
        if (config.clientSecret) {
            params.append('client_secret', config.clientSecret);
        }
        if (config.scope) {
            params.append('scope', config.scope);
        }

        this.log('Exchanging refresh token for access token...');
        const response = await fetch(config.tokenUrl, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/x-www-form-urlencoded',
                'Accept': 'application/json'
            },
            body: params.toString()
        });
        if (!response.ok) {
            const errorText = await response.text();
            throw new Error(`Token refresh failed: ${response.status} (${config.tokenUrl}) - ${errorDetail(errorText)}`);
        }

        const tokenResponse = await response.json() as TokenResponse;
        this.log('Token refresh successful');
        return {
            accessToken: tokenResponse.access_token,
            tokenType: tokenResponse.token_type,
            expiresIn: tokenResponse.expires_in,
            refreshToken: tokenResponse.refresh_token || refreshToken,
            scope: tokenResponse.scope
        };
    }
}
