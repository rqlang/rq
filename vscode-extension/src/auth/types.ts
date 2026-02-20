export interface OAuth2Config {
    clientId: string;
    clientSecret?: string;
    authorizationUrl: string;
    tokenUrl?: string;
    redirectUri: string;
    scope?: string;
    codeChallengeMethod?: 'S256' | 'plain';
    useState?: boolean;
    usePkce?: boolean;
}

export interface OAuth2Result {
    accessToken: string;
    tokenType?: string;
    expiresIn?: number;
    refreshToken?: string;
    scope?: string;
}

export interface IOAuth2Flow {
    execute(config: OAuth2Config): Promise<OAuth2Result>;
}

export interface TokenResponse {
    access_token: string;
    token_type: string;
    expires_in: number;
    refresh_token?: string;
    scope?: string;
}
