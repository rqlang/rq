import { AuthUi, OAuth2Config, OAuth2Result, IOAuth2Flow, TokenStore } from './types';
import { AuthorizationCodePKCEFlow } from './authorizationCodePKCEFlow';
import { ImplicitFlow } from './implicitFlow';
import { RefreshTokenFlow } from './refreshTokenFlow';

export type OAuth2FlowType = 'authorization_code_pkce' | 'implicit';

interface CachedToken {
    accessToken: string;
    expiresAt: number;
    refreshToken?: string;
    config: OAuth2Config;
}

const EXPIRY_MARGIN_MS = 60_000;
const DEFAULT_EXPIRES_IN_SECONDS = 3600;

export class OAuthProvider {
    private readonly flows: Record<OAuth2FlowType, IOAuth2Flow>;
    private readonly refreshTokenFlow: RefreshTokenFlow;
    private tokenCache = new Map<string, CachedToken>();
    private tokenCacheLoaded = false;

    constructor(private readonly store: TokenStore, private readonly ui: AuthUi) {
        this.flows = {
            authorization_code_pkce: new AuthorizationCodePKCEFlow(ui),
            implicit: new ImplicitFlow(ui)
        };
        this.refreshTokenFlow = new RefreshTokenFlow(message => ui.log(message));
    }

    async executeOAuth2Flow(config: OAuth2Config, flowType: OAuth2FlowType = 'authorization_code_pkce', authName?: string, environment?: string): Promise<OAuth2Result> {
        await this.loadTokenCache();
        const cacheKey = JSON.stringify({ authName: authName || '', environment: environment || '' });
        const cacheContext = environment ? `${authName} (env: ${environment})` : (authName || config.clientId);
        this.ui.log(`Checking token cache for ${cacheContext}`);

        const cachedToken = this.tokenCache.get(cacheKey);
        if (cachedToken && isTokenValid(cachedToken)) {
            const expiresInSeconds = Math.floor((cachedToken.expiresAt - Date.now()) / 1000);
            this.ui.log(`✓ Using cached OAuth2 token (expires in ${expiresInSeconds}s)`);
            return { accessToken: cachedToken.accessToken, refreshToken: cachedToken.refreshToken, expiresIn: expiresInSeconds };
        }

        if (cachedToken?.refreshToken) {
            try {
                this.ui.log('✗ Cached token expired, attempting to refresh it...');
                const refreshed = await this.refreshTokenFlow.execute(config, cachedToken.refreshToken);
                await this.cacheToken(cacheKey, config, refreshed);
                return refreshed;
            } catch (error) {
                this.ui.log(`⚠ Token refresh failed: ${error instanceof Error ? error.message : String(error)}. Falling back to full authentication flow`);
            }
        } else {
            this.ui.log(cachedToken ? '✗ Cached token expired and has no refresh token, performing OAuth2 flow' : '✗ No cached token found, performing OAuth2 flow');
        }

        const flow = this.flows[flowType];
        if (!flow) {
            throw new Error(`Unsupported OAuth2 flow type: ${flowType}`);
        }
        const result = await flow.execute(config);
        await this.cacheToken(cacheKey, config, result);
        return result;
    }

    async clearTokenCache(): Promise<void> {
        this.tokenCache.clear();
        await this.store.clear();
    }

    private async cacheToken(cacheKey: string, config: OAuth2Config, result: OAuth2Result): Promise<void> {
        const expiresIn = result.expiresIn || DEFAULT_EXPIRES_IN_SECONDS;
        const expiresAt = Date.now() + expiresIn * 1000;
        try {
            this.tokenCache = new Map([...this.tokenCache, ...await this.storedTokens()]);
        } catch (error) {
            this.ui.log(`Error loading token cache: ${error instanceof Error ? error.message : String(error)}`);
        }
        this.tokenCache.set(cacheKey, { accessToken: result.accessToken, expiresAt, refreshToken: result.refreshToken, config });
        try {
            await this.store.write(JSON.stringify(Array.from(this.tokenCache.entries())));
            this.ui.log(`✓ Token cached (expires in ${expiresIn}s at ${new Date(expiresAt).toLocaleTimeString()})`);
        } catch (error) {
            this.ui.log(`Error storing token cache: ${error instanceof Error ? error.message : String(error)}`);
        }
    }

    private async loadTokenCache(): Promise<void> {
        if (this.tokenCacheLoaded) {
            return;
        }
        this.tokenCacheLoaded = true;
        try {
            this.tokenCache = await this.storedTokens();
        } catch (error) {
            this.ui.log(`Error loading token cache: ${error instanceof Error ? error.message : String(error)}`);
            this.tokenCache = new Map();
        }
    }

    private async storedTokens(): Promise<Map<string, CachedToken>> {
        const cacheJson = await this.store.read();
        if (!cacheJson) {
            return new Map();
        }
        return new Map((JSON.parse(cacheJson) as [string, CachedToken][]).filter(([, token]) => isTokenValid(token) || token.refreshToken));
    }
}

function isTokenValid(token: CachedToken): boolean {
    return token.expiresAt > Date.now() + EXPIRY_MARGIN_MS;
}
