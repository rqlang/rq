import * as vscode from 'vscode';
import { OAuth2Config, OAuth2Result, IOAuth2Flow } from './types';
import { AuthorizationCodePKCEFlow } from './authorizationCodePKCEFlow';
import { ImplicitFlow } from './implicitFlow';
import { RefreshTokenFlow } from './refreshTokenFlow';

interface CachedToken {
    accessToken: string;
    expiresAt: number; // Unix timestamp
    refreshToken?: string;
    config: OAuth2Config;
}

export class OAuthProvider implements vscode.AuthenticationProvider, vscode.Disposable {
    private _sessions: vscode.AuthenticationSession[] = [];
    private _onDidChangeSessions = new vscode.EventEmitter<vscode.AuthenticationProviderAuthenticationSessionsChangeEvent>();
    public readonly onDidChangeSessions = this._onDidChangeSessions.event;
    private _disposables: vscode.Disposable[] = [];

    // OAuth flow implementations
    private readonly authorizationCodePKCEFlow: IOAuth2Flow;
    private readonly implicitFlow: IOAuth2Flow;
    private readonly refreshTokenFlow: RefreshTokenFlow;
    
    // Token cache keyed by a hash of the OAuth config
    private _tokenCache: Map<string, CachedToken> = new Map();
    private _tokenCacheLoaded: boolean = false;
    
    // Output channel for logging
    private _outputChannel?: vscode.OutputChannel;

    constructor(private readonly context: vscode.ExtensionContext) {
        this.authorizationCodePKCEFlow = new AuthorizationCodePKCEFlow();
        this.implicitFlow = new ImplicitFlow();
        this.refreshTokenFlow = new RefreshTokenFlow();
        this.loadSessions();
        // Don't await in constructor - will be loaded on first use
    }
    
    setOutputChannel(channel: vscode.OutputChannel): void {
        this._outputChannel = channel;
    }
    
    private log(message: string): void {
        console.log(message);
        if (this._outputChannel) {
            this._outputChannel.appendLine(`[OAuth] ${message}`);
        }
    }

    async executeOAuth2Flow(
        config: OAuth2Config,
        flowType: 'authorization_code_pkce' | 'implicit' = 'authorization_code_pkce',
        authName?: string,
        environment?: string
    ): Promise<OAuth2Result> {
        // Ensure cache is loaded
        if (!this._tokenCacheLoaded) {
            await this.loadTokenCache();
        }
        
        // Generate cache key based on config, auth name, and environment
        const cacheKey = this.getCacheKey(config, authName, environment);
        const cacheContext = environment ? `${authName} (env: ${environment})` : (authName || config.clientId);
        this.log(`Checking token cache for ${cacheContext}`);
        
        // Check if we have a valid cached token
        const cachedToken = this._tokenCache.get(cacheKey);
        if (cachedToken && this.isTokenValid(cachedToken)) {
            const expiresInSeconds = Math.floor((cachedToken.expiresAt - Date.now()) / 1000);
            this.log(`✓ Using cached OAuth2 token (expires in ${expiresInSeconds}s)`);
            return {
                accessToken: cachedToken.accessToken,
                refreshToken: cachedToken.refreshToken,
                expiresIn: expiresInSeconds
            };
        }
        
        // Token expired or not found, perform OAuth flow
        if (cachedToken) {
            this.log('✗ Cached token expired');
            
            // Try to refresh token if we have a refresh token
            if (cachedToken.refreshToken) {
                try {
                    this.log('Attempting to refresh token...');
                    const result = await this.refreshTokenFlow.execute(config, cachedToken.refreshToken);
                    this.log('✓ Token refreshed successfully');
                    
                    // Cache the new token
                    await this.cacheToken(cacheKey, config, result, authName, environment);
                    return result;
                } catch (error) {
                    this.log(`⚠ Token refresh failed: ${error instanceof Error ? error.message : String(error)}`);
                    this.log('Falling back to full authentication flow');
                    // Fall through to performFlow
                }
            } else {
                this.log('No refresh token available, performing new OAuth2 flow');
            }
        } else {
            this.log('✗ No cached token found, performing OAuth2 flow');
        }
        const result = await this.performFlow(config, flowType);
        
        // Cache the new token with auth name and environment
        await this.cacheToken(cacheKey, config, result, authName, environment);
        
        return result;
    }
    
    private async performFlow(
        config: OAuth2Config,
        flowType: 'authorization_code_pkce' | 'implicit'
    ): Promise<OAuth2Result> {
        switch (flowType) {
            case 'authorization_code_pkce':
                return await this.authorizationCodePKCEFlow.execute(config);
            case 'implicit':
                return await this.implicitFlow.execute(config);
            default:
                throw new Error(`Unsupported OAuth2 flow type: ${flowType}`);
        }
    }
    
    private getCacheKey(config: OAuth2Config, authName?: string, environment?: string): string {
        // Create a unique key based only on auth name and environment
        // This ensures each environment has its own cached token
        const keyData = {
            authName: authName || '',
            environment: environment || ''
        };
        return JSON.stringify(keyData);
    }
    
    private isTokenValid(token: CachedToken): boolean {
        // Add 60 second buffer before actual expiration
        const now = Date.now();
        return token.expiresAt > now + 60000;
    }
    
    private async cacheToken(
        cacheKey: string,
        config: OAuth2Config,
        result: OAuth2Result,
        authName?: string,
        environment?: string
    ): Promise<void> {
        // Calculate expiration time (default to 1 hour if not provided)
        const expiresIn = result.expiresIn || 3600;
        const expiresAt = Date.now() + (expiresIn * 1000);
        
        const cacheContext = environment ? `${authName} (env: ${environment})` : (authName || 'auth');
        this.log(`Caching token for ${cacheContext}: expiresIn=${expiresIn}s, expiresAt=${new Date(expiresAt).toISOString()}`);
        
        const cachedToken: CachedToken = {
            accessToken: result.accessToken,
            expiresAt,
            refreshToken: result.refreshToken,
            config
        };
        
        this._tokenCache.set(cacheKey, cachedToken);
        await this.storeTokenCache();
        
        this.log(`✓ Token cached successfully (expires in ${expiresIn}s at ${new Date(expiresAt).toLocaleTimeString()})`);
    }
    
    private async loadTokenCache(): Promise<void> {
        if (this._tokenCacheLoaded) {
            return; // Already loaded
        }
        
        try {
            const cacheJson = await this.context.secrets.get('oauth-token-cache');
            if (cacheJson) {
                const cacheArray: [string, CachedToken][] = JSON.parse(cacheJson);
                this._tokenCache = new Map(cacheArray);
                
                // Clean up expired tokens
                const initialSize = this._tokenCache.size;
                for (const [key, token] of this._tokenCache.entries()) {
                    if (!this.isTokenValid(token)) {
                        this._tokenCache.delete(key);
                    }
                }
                
                const expiredCount = initialSize - this._tokenCache.size;
                if (expiredCount > 0) {
                    this.log(`Loaded ${this._tokenCache.size} cached token(s), removed ${expiredCount} expired`);
                } else if (this._tokenCache.size > 0) {
                    this.log(`Loaded ${this._tokenCache.size} valid cached token(s)`);
                }
            }
            this._tokenCacheLoaded = true;
        } catch (error) {
            console.error('Error loading token cache:', error);
            this._tokenCache = new Map();
            this._tokenCacheLoaded = true;
        }
    }
    
    private async storeTokenCache(): Promise<void> {
        try {
            const cacheArray = Array.from(this._tokenCache.entries());
            await this.context.secrets.store('oauth-token-cache', JSON.stringify(cacheArray));
            console.log(`Stored ${this._tokenCache.size} cached token(s)`);
        } catch (error) {
            console.error('Error storing token cache:', error);
        }
    }
    
    async clearTokenCache(): Promise<void> {
        this._tokenCache.clear();
        await this.context.secrets.delete('oauth-token-cache');
        console.log('Token cache cleared');
    }

    async getSessions(scopes?: string[]): Promise<vscode.AuthenticationSession[]> {
        if (scopes) {
            return this._sessions.filter(session => 
                scopes.every(scope => session.scopes.includes(scope))
            );
        }
        return this._sessions;
    }

    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    async createSession(_scopes: string[]): Promise<vscode.AuthenticationSession> {
        throw new Error(
            'createSession is not supported. Use executeOAuth2Flow() with configuration from RQ CLI instead. ' +
            'OAuth configuration should come from your .rq files, not from hardcoded values.'
        );
    }

    async removeSession(sessionId: string): Promise<void> {
        console.log('Removing session:', sessionId);
        const sessionIndex = this._sessions.findIndex(s => s.id === sessionId);
        
        if (sessionIndex > -1) {
            const session = this._sessions[sessionIndex];
            this._sessions.splice(sessionIndex, 1);
            await this.storeSessions();

            this._onDidChangeSessions.fire({
                added: [],
                removed: [session],
                changed: []
            });

            vscode.window.showInformationMessage('Logged out successfully');
        }
    }

    private async loadSessions(): Promise<void> {
        try {
            const sessionsJson = await this.context.secrets.get('oauth-sessions');
            if (sessionsJson) {
                this._sessions = JSON.parse(sessionsJson);
                console.log(`Loaded ${this._sessions.length} session(s) from storage`);
            }
        } catch (error) {
            console.error('Error loading sessions:', error);
            this._sessions = [];
        }
    }

    private async storeSessions(): Promise<void> {
        try {
            await this.context.secrets.store('oauth-sessions', JSON.stringify(this._sessions));
            console.log(`Stored ${this._sessions.length} session(s)`);
        } catch (error) {
            console.error('Error storing sessions:', error);
        }
    }

    dispose(): void {
        this._disposables.forEach(d => d.dispose());
        this._onDidChangeSessions.dispose();
    }
}
