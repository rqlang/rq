import { RefreshTokenFlow } from '../../src/auth/refreshTokenFlow';
import { OAuth2Config } from '../../src/auth/types';
import { FetchMock } from '../fetch-mock';

describe('RefreshTokenFlow', () => {
    let refreshTokenFlow: RefreshTokenFlow;
    let fetchMock: FetchMock;

    const mockConfig: OAuth2Config = {
        clientId: 'test-client-id',
        authorizationUrl: 'https://auth.example.com/authorize',
        tokenUrl: 'https://auth.example.com/token',
        redirectUri: 'http://localhost:3000/callback',
        scope: 'read write'
    };

    beforeEach(() => {
        refreshTokenFlow = new RefreshTokenFlow();
        fetchMock = new FetchMock();
    });

    afterEach(() => {
        fetchMock.restore();
    });

    test('should successfully refresh token', async () => {
        const mockTokenResponse = {
            access_token: 'new-access-token',
            token_type: 'Bearer',
            expires_in: 3600,
            refresh_token: 'new-refresh-token',
            scope: 'read write'
        };

        fetchMock.mockResponse(mockTokenResponse);

        const result = await refreshTokenFlow.execute(mockConfig, 'old-refresh-token');

        expect(result).toEqual({
            accessToken: 'new-access-token',
            tokenType: 'Bearer',
            expiresIn: 3600,
            refreshToken: 'new-refresh-token',
            scope: 'read write'
        });

        const fetchCalls = fetchMock.jestMock.mock.calls;
        expect(fetchCalls.length).toBe(1);
        expect(fetchCalls[0][0]).toBe(mockConfig.tokenUrl);
        
        const requestInit = fetchCalls[0][1];
        expect(requestInit.method).toBe('POST');
        expect(requestInit.headers).toEqual({
            'Content-Type': 'application/x-www-form-urlencoded',
            'Accept': 'application/json'
        });

        const body = new URLSearchParams(requestInit.body);
        expect(body.get('grant_type')).toBe('refresh_token');
        expect(body.get('client_id')).toBe(mockConfig.clientId);
        expect(body.get('refresh_token')).toBe('old-refresh-token');
        expect(body.get('scope')).toBe(mockConfig.scope);
    });

    test('should include client secret if provided', async () => {
        const configWithSecret = { ...mockConfig, clientSecret: 'test-secret' };
        const mockTokenResponse = {
            access_token: 'new-access-token',
            token_type: 'Bearer',
            expires_in: 3600
        };

        fetchMock.mockResponse(mockTokenResponse);

        await refreshTokenFlow.execute(configWithSecret, 'old-refresh-token');

        const fetchCalls = fetchMock.jestMock.mock.calls;
        const requestInit = fetchCalls[0][1];
        const body = new URLSearchParams(requestInit.body);
        
        expect(body.get('client_secret')).toBe('test-secret');
    });

    test('should handle refresh failure', async () => {
        fetchMock.mockResponse({ error: 'invalid_grant' }, 400, false);

        await expect(refreshTokenFlow.execute(mockConfig, 'invalid-refresh-token'))
            .rejects.toThrow('Token refresh failed: undefined - {"error":"invalid_grant"}');
    });

    test('should handle network error', async () => {
        fetchMock.mockError(new Error('Network error'));

        await expect(refreshTokenFlow.execute(mockConfig, 'old-refresh-token'))
            .rejects.toThrow('Network error');
    });

    test('should reuse old refresh token if new one not provided', async () => {
        const mockTokenResponse = {
            access_token: 'new-access-token',
            token_type: 'Bearer',
            expires_in: 3600
            // No refresh_token in response
        };

        fetchMock.mockResponse(mockTokenResponse);

        const result = await refreshTokenFlow.execute(mockConfig, 'old-refresh-token');

        expect(result.refreshToken).toBe('old-refresh-token');
    });
});
