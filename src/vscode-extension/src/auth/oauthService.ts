import { OAuth2FlowType, OAuthProvider } from './oauthProvider';
import { OAuth2Config, OAuthAuthConfig } from './types';

export function getFlowTypeFromAuthType(authType: string): OAuth2FlowType {
    switch (authType) {
        case 'oauth2_authorization_code':
            return 'authorization_code_pkce';
        case 'oauth2_implicit':
            return 'implicit';
        default:
            throw new Error(
                `Auth type '${authType}' is not supported. ` +
                `Supported types: oauth2_authorization_code (OAuth2 with PKCE), oauth2_implicit`
            );
    }
}

export async function performOAuth2Flow(authConfig: OAuthAuthConfig, provider: OAuthProvider): Promise<string> {
    const flowType = getFlowTypeFromAuthType(authConfig.auth_type);
    const { fields } = authConfig;
    requireUrl('authorization_url', fields.authorization_url);
    if (fields.token_url) {
        requireUrl('token_url', fields.token_url);
    }

    const config: OAuth2Config = {
        clientId: fields.client_id,
        clientSecret: fields.client_secret || undefined,
        authorizationUrl: fields.authorization_url,
        tokenUrl: fields.token_url,
        redirectUri: fields.redirect_uri,
        scope: fields.scope || undefined,
        codeChallengeMethod: (fields.code_challenge_method as 'S256' | 'plain') || 'S256',
        useState: fields.use_state !== 'false',
        usePkce: fields.use_pkce !== 'false',
    };
    const result = await provider.executeOAuth2Flow(config, flowType, { authName: authConfig.name, environment: authConfig.environment, file: authConfig.file });
    return result.accessToken;
}

function requireUrl(field: string, value: string): void {
    try {
        new URL(value);
    } catch {
        throw new Error(`Invalid ${field}: "${value}". Make sure it's a valid URL with proper quotes in your .rq file.`);
    }
}
