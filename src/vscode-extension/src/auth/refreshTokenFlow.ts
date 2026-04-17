import { URLSearchParams } from 'url';
import { OAuth2Config, OAuth2Result, TokenResponse } from './types';

export class RefreshTokenFlow {
    async execute(config: OAuth2Config, refreshToken: string): Promise<OAuth2Result> {
        try {
            console.log('Starting OAuth2 Refresh Token flow');
            
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

            console.log('Exchanging refresh token for access token...');
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
                console.error('Token refresh failed:', response.status, response.statusText, errorText);
                throw new Error(`Token refresh failed: ${response.statusText} - ${errorText}`);
            }

            const tokenResponse = await response.json() as TokenResponse;
            console.log('Token refresh successful');

            return {
                accessToken: tokenResponse.access_token,
                tokenType: tokenResponse.token_type,
                expiresIn: tokenResponse.expires_in,
                refreshToken: tokenResponse.refresh_token || refreshToken, // Use new refresh token if provided, otherwise keep old one
                scope: tokenResponse.scope
            };
        } catch (error) {
            console.error('OAuth2 refresh token flow failed:', error);
            throw error;
        }
    }
}
