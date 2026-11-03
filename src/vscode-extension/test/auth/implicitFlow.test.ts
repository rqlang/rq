import { ImplicitFlow } from '../../src/auth/implicitFlow';
import { AuthUi, OAuth2Config } from '../../src/auth/types';
import * as callbackHandlers from '../../src/auth/callbackHandlers';
import { URL } from 'url';

jest.mock('../../src/auth/callbackHandlers', () => ({
    ...jest.requireActual('../../src/auth/callbackHandlers'),
    handleAppRedirectImplicitCallback: jest.fn(),
    handleManualPasteImplicitCallback: jest.fn()
}));

const ui: AuthUi = {
    openUrl: jest.fn(),
    askForRedirectUrl: jest.fn(),
    showError: jest.fn(),
    log: jest.fn()
};

describe('ImplicitFlow', () => {
    let implicitFlow: ImplicitFlow;

    const mockConfig: OAuth2Config = {
        clientId: 'test-client-id',
        authorizationUrl: 'https://auth.example.com/authorize',
        redirectUri: 'vscode://rq-lang.rq-language/callback',
        scope: 'read write'
    };

    beforeEach(() => {
        implicitFlow = new ImplicitFlow(ui);
        jest.clearAllMocks();
    });

    test('should construct correct authorization URL with response_type=token', async () => {
        (callbackHandlers.handleAppRedirectImplicitCallback as jest.Mock).mockImplementation((_ui: AuthUi, authUrl: string) => {
            const url = new URL(authUrl);
            expect(url.origin + url.pathname).toBe(mockConfig.authorizationUrl);
            expect(url.searchParams.get('client_id')).toBe(mockConfig.clientId);
            expect(url.searchParams.get('response_type')).toBe('token');
            expect(url.searchParams.get('redirect_uri')).toBe(mockConfig.redirectUri);
            expect(url.searchParams.get('scope')).toBe(mockConfig.scope);
            expect(url.searchParams.get('state')).toBeTruthy();
            
            return Promise.resolve({
                accessToken: 'test-access-token',
                tokenType: 'Bearer',
                expiresIn: 3600,
                state: url.searchParams.get('state')
            });
        });

        const result = await implicitFlow.execute(mockConfig);

        expect(callbackHandlers.handleAppRedirectImplicitCallback).toHaveBeenCalledTimes(1);
        expect(result.accessToken).toBe('test-access-token');
        expect(result.expiresIn).toBe(3600);
    });

    test('should fail if state mismatch', async () => {
        (callbackHandlers.handleAppRedirectImplicitCallback as jest.Mock).mockImplementation((_ui: AuthUi, authUrl: string) => {
            return Promise.resolve({
                accessToken: 'test-access-token',
                tokenType: 'Bearer',
                expiresIn: 3600,
                state: 'wrong-state'
            });
        });

        await expect(implicitFlow.execute(mockConfig)).rejects.toThrow('State parameter mismatch');
    });

    test('should throw error if invalid authorization URL', async () => {
        const invalidConfig = { ...mockConfig, authorizationUrl: 'not-a-url' };
        await expect(implicitFlow.execute(invalidConfig)).rejects.toThrow(/Invalid authorization URL/);
    });
    
    test('asks for the pasted redirect url when the redirect is not an app redirect', async () => {
        (callbackHandlers.handleManualPasteImplicitCallback as jest.Mock).mockImplementation((_ui: AuthUi, authUrl: string) =>
            Promise.resolve({ accessToken: 'pasted-token', state: new URL(authUrl).searchParams.get('state') })
        );

        const result = await implicitFlow.execute({ ...mockConfig, redirectUri: 'https://app.example.com/callback' });

        expect(result.accessToken).toBe('pasted-token');
    });
});
