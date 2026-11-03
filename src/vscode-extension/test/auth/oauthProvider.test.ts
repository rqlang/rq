import { OAuthProvider } from '../../src/auth/oauthProvider';
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

function cached(accessToken: string, expiresAt: number, refreshToken?: string): string {
    return JSON.stringify([[JSON.stringify({ authName: 'login', environment: '' }), { accessToken, expiresAt, refreshToken, config: CONFIG }]]);
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
        const result = await target.executeOAuth2Flow(CONFIG, 'authorization_code_pkce', 'login');
        expect([result.accessToken, fetchMock.jestMock.mock.calls.length]).toEqual(['stored', 0]);
    });

    test('runs the flow and writes the token to the store when none is cached', async () => {
        fetchMock.mockResponse({ access_token: 'fresh', token_type: 'Bearer', expires_in: 3600 });
        const store = memoryStore();
        await new OAuthProvider(store, ui).executeOAuth2Flow(CONFIG, 'authorization_code_pkce', 'login');
        expect(store.value).toContain('"fresh"');
    });

    test('refreshes an expired token kept in the store', async () => {
        fetchMock.mockResponse({ access_token: 'refreshed', token_type: 'Bearer', expires_in: 3600 });
        const target = new OAuthProvider(memoryStore(cached('expired', Date.now() - 1000, 'refresh-me')), ui);
        const result = await target.executeOAuth2Flow(CONFIG, 'authorization_code_pkce', 'login');
        expect([result.accessToken, (callbackHandlers.handleLocalServerCallback as jest.Mock).mock.calls.length]).toEqual(['refreshed', 0]);
    });

    test('clears the store', async () => {
        const store = memoryStore(cached('stored', Date.now() + 3_600_000));
        await new OAuthProvider(store, ui).clearTokenCache();
        expect(store.value).toBeUndefined();
    });
});
