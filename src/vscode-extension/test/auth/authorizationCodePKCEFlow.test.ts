import { AuthorizationCodePKCEFlow } from '../../src/auth/authorizationCodePKCEFlow';
import { AuthUi, OAuth2Config } from '../../src/auth/types';
import { FetchMock } from '../fetch-mock';
import * as callbackHandlers from '../../src/auth/callbackHandlers';
import { URL } from 'url';

jest.mock('../../src/auth/callbackHandlers', () => ({
    ...jest.requireActual('../../src/auth/callbackHandlers'),
    handleAppRedirectCallback: jest.fn(),
    handleLocalServerCallback: jest.fn(),
    handleManualPasteCallback: jest.fn()
}));

const ui: AuthUi = {
    openUrl: jest.fn(),
    askForRedirectUrl: jest.fn(),
    showError: jest.fn(),
    log: jest.fn()
};

describe('AuthorizationCodePKCEFlow', () => {
    let pkceFlow: AuthorizationCodePKCEFlow;
    let fetchMock: FetchMock;

    const mockConfig: OAuth2Config = {
        clientId: 'test-client-id',
        authorizationUrl: 'https://auth.example.com/authorize',
        tokenUrl: 'https://auth.example.com/token',
        redirectUri: 'http://localhost:3000/callback',
        scope: 'read write'
    };

    beforeEach(() => {
        pkceFlow = new AuthorizationCodePKCEFlow(ui);
        fetchMock = new FetchMock();
        
        (callbackHandlers.handleLocalServerCallback as jest.Mock).mockImplementation((_ui: AuthUi, authUrl: string) => {
            const url = new URL(authUrl);
            const state = url.searchParams.get('state');
            return Promise.resolve({ code: 'test-auth-code', state });
        });
    });

    afterEach(() => {
        fetchMock.restore();
        jest.clearAllMocks();
    });

    test('should NOT include client_secret when not defined in config', async () => {
        const mockTokenResponse = {
            access_token: 'access-token',
            token_type: 'Bearer',
            expires_in: 3600
        };
        fetchMock.mockResponse(mockTokenResponse);

        await pkceFlow.execute(mockConfig);

        const fetchCalls = fetchMock.jestMock.mock.calls;
        expect(fetchCalls.length).toBe(1);
        
        const requestInit = fetchCalls[0][1];
        const body = new URLSearchParams(requestInit.body);
        
        expect(body.get('client_id')).toBe(mockConfig.clientId);
        expect(body.get('code')).toBe('test-auth-code');
        expect(body.has('client_secret')).toBe(false);
    });

    test('should include client_secret when defined in config', async () => {
        const configWithSecret = { ...mockConfig, clientSecret: 'my-secret' };
        
        const mockTokenResponse = {
            access_token: 'access-token',
            token_type: 'Bearer',
            expires_in: 3600
        };
        fetchMock.mockResponse(mockTokenResponse);

        await pkceFlow.execute(configWithSecret);

        const fetchCalls = fetchMock.jestMock.mock.calls;
        expect(fetchCalls.length).toBe(1);
        
        const requestInit = fetchCalls[0][1];
        const body = new URLSearchParams(requestInit.body);
        
        expect(body.get('client_id')).toBe(mockConfig.clientId);
        expect(body.get('client_secret')).toBe('my-secret');
    });

    test('should include state parameter by default', async () => {
        const mockTokenResponse = {
            access_token: 'access-token',
            token_type: 'Bearer',
            expires_in: 3600
        };
        fetchMock.mockResponse(mockTokenResponse);

        await pkceFlow.execute(mockConfig);

        // Check that handleLocalServerCallback was called with a URL containing state
        const authUrl = (callbackHandlers.handleLocalServerCallback as jest.Mock).mock.calls[0][1];
        const url = new URL(authUrl);
        expect(url.searchParams.has('state')).toBe(true);
    });

    test('should NOT include state parameter when useState is false', async () => {
        const configNoState = { ...mockConfig, useState: false };
        
        const mockTokenResponse = {
            access_token: 'access-token',
            token_type: 'Bearer',
            expires_in: 3600
        };
        fetchMock.mockResponse(mockTokenResponse);

        await pkceFlow.execute(configNoState);

        // Check that handleLocalServerCallback was called with a URL NOT containing state
        const authUrl = (callbackHandlers.handleLocalServerCallback as jest.Mock).mock.calls[0][1];
        const url = new URL(authUrl);
        expect(url.searchParams.has('state')).toBe(false);
    });

    test('should fail if state mismatch', async () => {
        // Mock callback to return wrong state
        (callbackHandlers.handleLocalServerCallback as jest.Mock).mockResolvedValue({ 
            code: 'test-auth-code', 
            state: 'wrong-state' 
        });

        await expect(pkceFlow.execute(mockConfig)).rejects.toThrow('comes from another sign-in');
    });

    test('explains a callback without state, the sign of a cut-off sign-in link', async () => {
        (callbackHandlers.handleLocalServerCallback as jest.Mock).mockResolvedValue({ code: 'test-auth-code', state: null });
        await expect(pkceFlow.execute(mockConfig)).rejects.toThrow('cut off before its state');
    });

    test('explains a rejected code exchange', async () => {
        (callbackHandlers.handleLocalServerCallback as jest.Mock).mockImplementation((_ui: unknown, authUrl: string) =>
            Promise.resolve({ code: 'test-auth-code', state: new URL(authUrl).searchParams.get('state') })
        );
        fetchMock.mockResponse({ error: 'invalid_grant', error_description: 'Code not valid' }, 400, false);
        await expect(pkceFlow.execute(mockConfig)).rejects.toThrow('rejected the authorization code at https://auth.example.com/token (HTTP 400: Code not valid)');
    });

});
