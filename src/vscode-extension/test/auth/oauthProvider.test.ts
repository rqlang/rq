import { OAuthProvider, tokenCacheKey } from '../../src/auth/oauthProvider';
import { AuthUi, OAuth2Config, TokenStore } from '../../src/auth/types';
import { FetchMock } from '../fetch-mock';
import * as callbackHandlers from '../../src/auth/callbackHandlers';

jest.mock('../../src/auth/callbackHandlers', () => ({
    ...jest.requireActual('../../src/auth/callbackHandlers'),
    handleLocalServerCallback: jest.fn()
}));

const CONFIG: OAuth2Config = {
    clientId: 'app',
    authorizationUrl: 'https://auth.example.com/authorize',
    tokenUrl: 'https://auth.example.com/token',
    redirectUri: 'http://localhost:3000/callback'
};

const ui: AuthUi = { openUrl: jest.fn(), askForRedirectUrl: jest.fn(), showError: jest.fn(), log: jest.fn() };

function memoryStore(initial?: string): TokenStore & { value?: string } {
    const store: TokenStore & { value?: string } = {
        value: initial,
        read: async () => store.value,
        write: async value => { store.value = value; },
        clear: async () => { store.value = undefined; }
    };
    return store;
}

const SCOPE = { authName: 'login', file: '/workspace/api/shared.rq' };

function cached(accessToken: string, expiresAt: number, refreshToken?: string, scope = SCOPE): string {
    return JSON.stringify([[tokenCacheKey(scope), { accessToken, expiresAt, refreshToken, config: CONFIG }]]);
}

describe('OAuthProvider', () => {
    let fetchMock: FetchMock;

    beforeEach(() => {
        fetchMock = new FetchMock();
        (callbackHandlers.handleLocalServerCallback as jest.Mock).mockImplementation((_ui: AuthUi, authUrl: string) =>
            Promise.resolve({ code: 'code', state: new URL(authUrl).searchParams.get('state') })
        );
    });

    afterEach(() => {
        fetchMock.restore();
        jest.clearAllMocks();
    });

    test('reuses a valid token from the store without running the flow', async () => {
        const target = new OAuthProvider(memoryStore(cached('stored', Date.now() + 3_600_000)), ui);
        const result = await target.executeOAuth2Flow(CONFIG, 'authorization_code_pkce', SCOPE);
        expect([result.accessToken, fetchMock.jestMock.mock.calls.length]).toEqual(['stored', 0]);
    });

    test('runs the flow and writes the token to the store when none is cached', async () => {
        fetchMock.mockResponse({ access_token: 'fresh', token_type: 'Bearer', expires_in: 3600 });
        const store = memoryStore();
        await new OAuthProvider(store, ui).executeOAuth2Flow(CONFIG, 'authorization_code_pkce', SCOPE);
        expect(store.value).toContain('"fresh"');
    });

    test('refreshes an expired token kept in the store', async () => {
        fetchMock.mockResponse({ access_token: 'refreshed', token_type: 'Bearer', expires_in: 3600 });
        const target = new OAuthProvider(memoryStore(cached('expired', Date.now() - 1000, 'refresh-me')), ui);
        const result = await target.executeOAuth2Flow(CONFIG, 'authorization_code_pkce', SCOPE);
        expect([result.accessToken, (callbackHandlers.handleLocalServerCallback as jest.Mock).mock.calls.length]).toEqual(['refreshed', 0]);
    });

    test('keeps a token another provider stored while its own flow was running', async () => {
        fetchMock.mockResponse({ access_token: 'fresh', token_type: 'Bearer', expires_in: 3600 });
        const store = memoryStore();
        const target = new OAuthProvider(store, ui);
        (callbackHandlers.handleLocalServerCallback as jest.Mock).mockImplementationOnce(async (_ui: AuthUi, authUrl: string) => {
            store.value = cached('other', Date.now() + 3_600_000);
            return { code: 'code', state: new URL(authUrl).searchParams.get('state') };
        });
        await target.executeOAuth2Flow(CONFIG, 'authorization_code_pkce', { ...SCOPE, authName: 'mine' });
        expect([store.value?.includes('"other"'), store.value?.includes('"fresh"')]).toEqual([true, true]);
    });

    test('does not reuse a token cached for an auth of the same name in another file', async () => {
        fetchMock.mockResponse({ access_token: 'fresh', token_type: 'Bearer', expires_in: 3600 });
        const target = new OAuthProvider(memoryStore(cached('stored', Date.now() + 3_600_000)), ui);
        const result = await target.executeOAuth2Flow(CONFIG, 'authorization_code_pkce', { ...SCOPE, file: '/other/api/shared.rq' });
        expect(result.accessToken).toBe('fresh');
    });

    test('clears the store', async () => {
        const store = memoryStore(cached('stored', Date.now() + 3_600_000));
        await new OAuthProvider(store, ui).clearTokenCache();
        expect(store.value).toBeUndefined();
    });
});

describe('tokenCacheKey', () => {
    test('tells apart auths of the same name and environment in different files', () => {
        expect(tokenCacheKey({ ...SCOPE, file: '/a/shared.rq' })).not.toBe(tokenCacheKey({ ...SCOPE, file: '/b/shared.rq' }));
    });

    test('tells apart environments of the same auth', () => {
        expect(tokenCacheKey({ ...SCOPE, environment: 'dev' })).not.toBe(tokenCacheKey({ ...SCOPE, environment: 'prod' }));
    });

    test('gives the same key to two spellings of the same file', () => {
        expect(tokenCacheKey({ ...SCOPE, file: '/workspace/api/../api/shared.rq' })).toBe(tokenCacheKey(SCOPE));
    });
});
