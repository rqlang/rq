import { AuthorizationCodePKCEFlow } from '../../src/auth/authorizationCodePKCEFlow';
import { OAuth2Config } from '../../src/auth/types';
import { FetchMock } from '../fetch-mock';
import * as callbackHandlers from '../../src/auth/callbackHandlers';
import { URL } from 'url';

// Mock the callback handlers
jest.mock('../../src/auth/callbackHandlers', () => ({
    handleVSCodeUriCallback: jest.fn(),
    handleLocalServerCallback: jest.fn(),
    handleManualPasteCallback: jest.fn()
}));

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
        pkceFlow = new AuthorizationCodePKCEFlow();
        fetchMock = new FetchMock();
        
        // Setup default mock for callback handler to echo back the state from the URL
        (callbackHandlers.handleLocalServerCallback as jest.Mock).mockImplementation((authUrl: string) => {
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
        const authUrl = (callbackHandlers.handleLocalServerCallback as jest.Mock).mock.calls[0][0];
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
        const authUrl = (callbackHandlers.handleLocalServerCallback as jest.Mock).mock.calls[0][0];
        const url = new URL(authUrl);
        expect(url.searchParams.has('state')).toBe(false);
    });

    test('should fail if state mismatch', async () => {
        // Mock callback to return wrong state
        (callbackHandlers.handleLocalServerCallback as jest.Mock).mockResolvedValue({ 
            code: 'test-auth-code', 
            state: 'wrong-state' 
        });

        await expect(pkceFlow.execute(mockConfig)).rejects.toThrow('State parameter mismatch');
    });

    test('should NOT include code_challenge when usePkce is false', async () => {
        const configNoPkce = { ...mockConfig, usePkce: false };
        
        const mockTokenResponse = {
            access_token: 'access-token',
            token_type: 'Bearer',
            expires_in: 3600
        };
        fetchMock.mockResponse(mockTokenResponse);

        await pkceFlow.execute(configNoPkce);

        // Check auth URL
        const authUrl = (callbackHandlers.handleLocalServerCallback as jest.Mock).mock.calls[0][0];
        const url = new URL(authUrl);
        expect(url.searchParams.has('code_challenge')).toBe(false);
        expect(url.searchParams.has('code_challenge_method')).toBe(false);

        // Check token exchange
        const fetchCalls = fetchMock.jestMock.mock.calls;
        expect(fetchCalls.length).toBe(1);
        
        const requestInit = fetchCalls[0][1];
        const body = new URLSearchParams(requestInit.body);
        expect(body.has('code_verifier')).toBe(false);
    });
});
