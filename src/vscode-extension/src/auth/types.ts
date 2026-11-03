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

export interface RedirectUrlRequest {
    authorizationUrl: string;
    prompt: string;
    placeholder?: string;
    validate?: (value: string) => string | undefined;
}

export interface AuthUi {
    openUrl(url: string): Promise<void>;
    askForRedirectUrl(request: RedirectUrlRequest): Promise<string | undefined>;
    waitForAppRedirect?(accepts: (url: URL) => boolean): Promise<URL>;
    showError(message: string): void;
    log(message: string): void;
}

export interface TokenStore {
    read(): Promise<string | undefined>;
    write(value: string): Promise<void>;
    clear(): Promise<void>;
}

export interface OAuthAuthConfig {
    name: string;
    auth_type: string;
    fields: Record<string, string>;
    environment?: string;
}
