import * as http from 'http';
import { Authorization, AuthorizationUrlOutcome, Authorizer, isInteractiveAuth, memoryTokenStore } from '../../src/mcp/authorization';
import { OAuthAuthConfig } from '../../src/auth/types';
import { CANCELLED_MESSAGE } from '../../src/utils';
import { FetchMock } from '../fetch-mock';

const REDIRECT_URI = 'http://127.0.0.1:47392/callback';

function authConfig(fields: Record<string, string> = {}, authType = 'oauth2_authorization_code'): OAuthAuthConfig {
    return {
        name: 'login',
        auth_type: authType,
        fields: {
            client_id: 'app',
            authorization_url: 'https://auth.example.com/authorize',
            token_url: 'https://auth.example.com/token',
            redirect_uri: REDIRECT_URI,
            ...fields
        }
    };
}

function signIn(authorizationUrl: string): Promise<void> {
    const state = new URL(authorizationUrl).searchParams.get('state');
    return new Promise((resolve, reject) => {
        http.get(`${REDIRECT_URI}?code=granted&state=${state}`, res => {
            res.resume();
            res.on('end', () => resolve());
        }).on('error', reject);
    });
}

function opener(outcome: AuthorizationUrlOutcome, onOpen: (url: string) => Promise<void> = async () => undefined) {
    return jest.fn(async (url: string) => {
        await onOpen(url);
        return outcome;
    });
}

async function signedIn(target: Authorizer, first: Authorization): Promise<Authorization> {
    if (first.status !== 'authorization_required') {
        throw new Error(`Expected authorization_required, got ${first.status}`);
    }
    await signIn(first.authorizationUrl);
    return target.authorize(authConfig());
}

describe('Authorizer', () => {
    let fetchMock: FetchMock;

    beforeEach(() => {
        fetchMock = new FetchMock();
        fetchMock.mockResponse({ access_token: 'granted-token', token_type: 'Bearer', expires_in: 3600 });
    });

    afterEach(() => {
        fetchMock.restore();
    });

    it('returns the authorization url when the client cannot open it', async () => {
        const target = new Authorizer(opener('unavailable'));
        const first = await target.authorize(authConfig());
        await signedIn(target, first);
        expect(first.status === 'authorization_required' && new URL(first.authorizationUrl).searchParams.get('redirect_uri')).toBe(REDIRECT_URI);
    });

    it('returns the token on the call after the user signed in', async () => {
        const target = new Authorizer(opener('unavailable'));
        expect(await signedIn(target, await target.authorize(authConfig()))).toEqual({ status: 'authorized', token: 'granted-token' });
    });

    it('returns the same url again while the sign-in is still pending', async () => {
        const target = new Authorizer(opener('unavailable'), undefined, 10);
        const first = await target.authorize(authConfig());
        const second = await target.authorize(authConfig());
        await signedIn(target, first);
        expect(second).toEqual(first);
    });

    it('waits for the sign-in when the client opens the url', async () => {
        const target = new Authorizer(opener('accepted', signIn));
        expect(await target.authorize(authConfig())).toEqual({ status: 'authorized', token: 'granted-token' });
    });

    it('tells the opener once the sign-in finished', async () => {
        let finished: Promise<void> | undefined;
        const target = new Authorizer(jest.fn(async (url: string, _auth: OAuthAuthConfig, done: Promise<void>) => {
            finished = done;
            await signIn(url);
            return 'accepted' as const;
        }));
        await target.authorize(authConfig());
        await expect(finished).resolves.toBeUndefined();
    });

    it('reuses the token it holds without opening the url again', async () => {
        const open = opener('accepted', signIn);
        const target = new Authorizer(open);
        await target.authorize(authConfig());
        await target.authorize(authConfig());
        expect(open).toHaveBeenCalledTimes(1);
    });

    it('reports a declined sign-in', async () => {
        const target = new Authorizer(opener('declined'));
        expect(await target.authorize(authConfig())).toEqual({ status: 'declined' });
    });

    it('starts a new sign-in after the user declined one', async () => {
        const open = opener('declined');
        const target = new Authorizer(open);
        await target.authorize(authConfig());
        await target.authorize(authConfig());
        expect(open).toHaveBeenCalledTimes(2);
    });

    it('rejects a redirect uri it cannot receive before opening anything', async () => {
        const open = opener('accepted');
        const target = new Authorizer(open);
        await expect(target.authorize(authConfig({ redirect_uri: 'https://app.example.com/callback' }))).rejects.toThrow('https://app.example.com/callback');
        expect(open).not.toHaveBeenCalled();
    });

    it('rejects an auth with no redirect uri, which defaults to VS Code', async () => {
        const target = new Authorizer(opener('accepted'));
        await expect(target.authorize(authConfig({ redirect_uri: '' }))).rejects.toThrow('the VS Code default');
    });

    it('rejects the implicit flow', async () => {
        const target = new Authorizer(opener('accepted'));
        await expect(target.authorize(authConfig({}, 'oauth2_implicit'))).rejects.toThrow('implicit flow');
    });

    it('stops waiting once the call is cancelled', async () => {
        const cancellation = new AbortController();
        let url = '';
        const target = new Authorizer(opener('accepted', async opened => {
            url = opened;
            cancellation.abort();
        }));
        await expect(target.authorize(authConfig(), cancellation.signal)).rejects.toThrow(CANCELLED_MESSAGE);
        await signIn(url);
    });
});

describe('isInteractiveAuth', () => {
    it('is true for the flows that need a browser sign-in', () => {
        expect(['oauth2_authorization_code', 'oauth2_implicit'].map(isInteractiveAuth)).toEqual([true, true]);
    });

    it('is false for flows rq can run on its own', () => {
        expect(['bearer', 'oauth2_client_credentials'].map(isInteractiveAuth)).toEqual([false, false]);
    });
});

describe('memoryTokenStore', () => {
    it('reads back what it was given', async () => {
        const target = memoryTokenStore();
        await target.write('cache');
        expect(await target.read()).toBe('cache');
    });

    it('forgets the value once cleared', async () => {
        const target = memoryTokenStore();
        await target.write('cache');
        await target.clear();
        expect(await target.read()).toBeUndefined();
    });
});
