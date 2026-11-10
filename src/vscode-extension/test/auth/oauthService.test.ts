import { OAuthProvider } from '../../src/auth/oauthProvider';
import { performOAuth2Flow } from '../../src/auth/oauthService';

describe('performOAuth2Flow', () => {
    test('rejects an auth without redirect_uri before starting a sign-in', async () => {
        const executeOAuth2Flow = jest.fn();
        const provider = { executeOAuth2Flow } as unknown as OAuthProvider;

        const target = performOAuth2Flow({
            name: 'login',
            auth_type: 'oauth2_implicit',
            fields: { client_id: 'app', authorization_url: 'https://auth.example.com/authorize' }
        }, provider);

        await expect(target).rejects.toThrow("Auth 'login' has no redirect_uri");
        expect(executeOAuth2Flow).not.toHaveBeenCalled();
    });
});
