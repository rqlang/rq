import * as http from 'http';
import { Authorization, AuthorizationPrompts, AuthorizationUrlOutcome, Authorizer, isInteractiveAuth, memoryTokenStore, RedirectUrlOutcome } from '../../src/mcp/authorization';
import { OAuthAuthConfig } from '../../src/auth/types';
import { CANCELLED_MESSAGE } from '../../src/utils';
import { FetchMock } from '../fetch-mock';

const REDIRECT_URI = 'http://127.0.0.1:47392/callback';

function authConfig(fields: Record<string, string> = {}, authType = 'oauth2_authorization_code', file = '/workspace/shared.rq'): OAuthAuthConfig {
    return {
        name: 'login',
        auth_type: authType,
        file,
        fields: {
            client_id: 'app',
            authorization_url: 'https://auth.example.com/authorize',
            token_url: 'https://auth.example.com/token',
            redirect_uri: REDIRECT_URI,
            ...fields
        }
    };
}

function followLink(url: string): Promise<string> {
    return new Promise((resolve, reject) => {
        http.get(url, res => {
            res.resume();
            res.on('end', () => resolve(res.headers.location ?? ''));
        }).on('error', reject);
    });
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

function withOpener(openUrl: AuthorizationPrompts['openUrl'], log?: (message: string) => void, retryGraceMs?: number): Authorizer {
    return new Authorizer({ openUrl, askForRedirectUrl: async () => ({ status: 'unavailable' }) }, log, retryGraceMs);
}

async function signedIn(target: Authorizer, first: Authorization): Promise<Authorization> {
    if (first.status !== 'authorization_required') {
        throw new Error(`Expected authorization_required, got ${first.status}`);
    }
    await signIn(await followLink(first.authorizationUrl));
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
        const target = withOpener(opener('unavailable'));
        const first = await target.authorize(authConfig());
        const authorizationUrl = first.status === 'authorization_required' ? await followLink(first.authorizationUrl) : '';
        await signedIn(target, first);
        expect(new URL(authorizationUrl).searchParams.get('redirect_uri')).toBe(REDIRECT_URI);
    });

    it('returns the token on the call after the user signed in', async () => {
        const target = withOpener(opener('unavailable'));
        expect(await signedIn(target, await target.authorize(authConfig()))).toEqual({ status: 'authorized', token: 'granted-token' });
    });

    it('returns the same url again while the sign-in is still pending', async () => {
        const target = withOpener(opener('unavailable'), undefined, 10);
        const first = await target.authorize(authConfig());
        const second = await target.authorize(authConfig());
        await signedIn(target, first);
        expect(second).toEqual(first);
    });

    it('waits for the sign-in when the client opens the url', async () => {
        const target = withOpener(opener('accepted', signIn));
        expect(await target.authorize(authConfig())).toEqual({ status: 'authorized', token: 'granted-token' });
    });

    it('tells the opener once the sign-in finished', async () => {
        let finished: Promise<void> | undefined;
        const target = withOpener(jest.fn(async (url: string, _auth: OAuthAuthConfig, done: Promise<void>) => {
            finished = done;
            await signIn(url);
            return 'accepted' as const;
        }));
        await target.authorize(authConfig());
        await expect(finished).resolves.toBeUndefined();
    });

    it('reuses the token it holds without opening the url again', async () => {
        const open = opener('accepted', signIn);
        const target = withOpener(open);
        await target.authorize(authConfig());
        await target.authorize(authConfig());
        expect(open).toHaveBeenCalledTimes(1);
    });

    it('asks to sign in again for an auth of the same name in another workspace', async () => {
        const open = opener('accepted', signIn);
        const target = withOpener(open);
        await target.authorize(authConfig());
        await target.authorize(authConfig({}, 'oauth2_authorization_code', '/other/shared.rq'));
        expect(open).toHaveBeenCalledTimes(2);
    });

    it('reports a declined sign-in', async () => {
        const target = withOpener(opener('declined'));
        expect(await target.authorize(authConfig())).toEqual({ status: 'declined' });
    });

    it('reports a sign-in dialog nobody answered in time as a timeout, not a refusal', async () => {
        await expect(withOpener(opener('unanswered')).authorize(authConfig())).rejects.toThrow('No sign-in finished within 5 minutes');
    });

    it('starts a new sign-in after the user declined one', async () => {
        const open = opener('declined');
        const target = withOpener(open);
        await target.authorize(authConfig());
        await target.authorize(authConfig());
        expect(open).toHaveBeenCalledTimes(2);
    });

    it('rejects a redirect uri only VS Code can receive before opening anything', async () => {
        const open = opener('accepted');
        const target = withOpener(open);
        await expect(target.authorize(authConfig({ redirect_uri: 'vscode://rq-lang.rq-language/oauth-callback' }))).rejects.toThrow('vscode://rq-lang.rq-language/oauth-callback');
        expect(open).not.toHaveBeenCalled();
    });

    it('rejects an auth with no redirect uri, which defaults to VS Code', async () => {
        const target = withOpener(opener('accepted'));
        await expect(target.authorize(authConfig({ redirect_uri: '' }))).rejects.toThrow('the VS Code default');
    });

    it('stops waiting once the call is cancelled', async () => {
        const cancellation = new AbortController();
        let url = '';
        const target = withOpener(opener('accepted', async opened => {
            url = opened;
            setTimeout(() => cancellation.abort(), 0);
        }));
        await expect(target.authorize(authConfig(), cancellation.signal)).rejects.toThrow(CANCELLED_MESSAGE);
        await signIn(url);
    });

    it('keeps the dialog open for a call still waiting on it when another is cancelled', async () => {
        const dialog = openDialog();
        const target = withOpener(dialog.openUrl);
        const cancellation = new AbortController();
        const first = target.authorize(authConfig(), cancellation.signal).catch(() => undefined);
        const second = target.authorize(authConfig());
        cancellation.abort();
        await first;
        await signIn(await dialog.opened);
        dialog.answer('accepted');
        expect([await second, dialog.signal()?.aborted]).toEqual([{ status: 'authorized', token: 'granted-token' }, false]);
    });

    it('closes the dialog once every call waiting on it is cancelled', async () => {
        const dialog = openDialog();
        const cancellation = new AbortController();
        const call = withOpener(dialog.openUrl).authorize(authConfig(), cancellation.signal);
        await dialog.opened;
        cancellation.abort();
        await expect(call).rejects.toThrow(CANCELLED_MESSAGE);
        expect(dialog.signal()?.aborted).toBe(true);
    });

    it('asks again on the next call after the only one waiting on the dialog was cancelled', async () => {
        const dialog = openDialog();
        const target = withOpener(dialog.openUrl);
        const cancellation = new AbortController();
        const call = target.authorize(authConfig(), cancellation.signal);
        await dialog.opened;
        cancellation.abort();
        await expect(call).rejects.toThrow(CANCELLED_MESSAGE);
        target.authorize(authConfig()).catch(() => undefined);
        await new Promise(resolve => setTimeout(resolve, 20));
        expect(dialog.openUrl).toHaveBeenCalledTimes(2);
    });
});

function openDialog() {
    let answer: (outcome: AuthorizationUrlOutcome) => void = () => undefined;
    let opened: (url: string) => void = () => undefined;
    let signal: AbortSignal | undefined;
    const openUrl = jest.fn((url: string, _auth: OAuthAuthConfig, _finished: Promise<void>, received?: AbortSignal) => {
        signal = received;
        opened(url);
        return new Promise<AuthorizationUrlOutcome>(resolve => {
            answer = resolve;
            received?.addEventListener('abort', () => resolve('declined'));
        });
    });
    return {
        openUrl,
        opened: new Promise<string>(resolve => { opened = resolve; }),
        answer: (outcome: AuthorizationUrlOutcome) => answer(outcome),
        signal: () => signal
    };
}

const PASTED_REDIRECT_URI = 'https://app.example.com/callback';

function pastedAuth(fields: Record<string, string> = {}, authType = 'oauth2_authorization_code'): OAuthAuthConfig {
    return authConfig({ redirect_uri: PASTED_REDIRECT_URI, ...fields }, authType);
}

function landedOn(authorizationUrl: string, extra = ''): string {
    const state = new URL(authorizationUrl).searchParams.get('state');
    return `${PASTED_REDIRECT_URI}?code=granted&state=${state}${extra}`;
}

function withDialog(answer: (authorizationUrl: string) => RedirectUrlOutcome, openUrl = opener('accepted')): Authorizer {
    return new Authorizer({ openUrl, askForRedirectUrl: async url => answer(await followLink(url)) });
}

async function pastedInChat(target: Authorizer, auth: OAuthAuthConfig, land = landedOn): Promise<Authorization> {
    const first = await target.authorize(auth);
    if (first.status !== 'authorization_required') {
        throw new Error(`Expected authorization_required, got ${first.status}`);
    }
    return target.authorize(auth, undefined, land(await followLink(first.authorizationUrl)));
}

describe('Authorizer with a redirect address the user pastes', () => {
    let fetchMock: FetchMock;

    beforeEach(() => {
        fetchMock = new FetchMock();
        fetchMock.mockResponse({ access_token: 'granted-token', token_type: 'Bearer', expires_in: 3600 });
    });

    afterEach(() => {
        fetchMock.restore();
    });

    it('takes the address pasted in the dialog and returns the token in the same call', async () => {
        const target = withDialog(url => ({ status: 'pasted', url: landedOn(url) }));
        expect(await target.authorize(pastedAuth())).toEqual({ status: 'authorized', token: 'granted-token' });
    });

    it('does not ask the client to open the url, since the dialog shows it', async () => {
        const open = opener('accepted');
        await withDialog(url => ({ status: 'pasted', url: landedOn(url) }), open).authorize(pastedAuth());
        expect(open).not.toHaveBeenCalled();
    });

    it('reports a dialog the user dismissed as declined', async () => {
        expect(await withDialog(() => ({ status: 'declined' })).authorize(pastedAuth())).toEqual({ status: 'declined' });
    });

    it('reports a paste dialog nobody answered in time as a timeout, not a refusal', async () => {
        await expect(withDialog(() => ({ status: 'unanswered' })).authorize(pastedAuth())).rejects.toThrow('No sign-in finished within 5 minutes');
    });

    it('rejects an address from the dialog that is not the redirect uri', async () => {
        const target = withDialog(url => ({ status: 'pasted', url: landedOn(url).replace(PASTED_REDIRECT_URI, 'https://evil.example.com/callback') }));
        await expect(target.authorize(pastedAuth())).rejects.toThrow(`does not start with the redirect_uri ${PASTED_REDIRECT_URI}`);
    });

    it('takes the token straight from the address pasted in the dialog for the implicit flow', async () => {
        const target = withDialog(url => ({ status: 'pasted', url: `${PASTED_REDIRECT_URI}#access_token=implicit-token&state=${new URL(url).searchParams.get('state')}` }));
        expect(await target.authorize(pastedAuth({}, 'oauth2_implicit'))).toEqual({ status: 'authorized', token: 'implicit-token' });
    });

    it('shows the dialog a short link to the sign-in page', async () => {
        const shown: string[] = [];
        const target = new Authorizer({ openUrl: opener('accepted'), askForRedirectUrl: async url => {
            shown.push(url);
            return { status: 'pasted', url: landedOn(await followLink(url)) };
        } });
        await target.authorize(pastedAuth());
        expect(shown[0]).toMatch(/^http:\/\/127\.0\.0\.1:\d+\/[0-9a-f]{32}$/);
    });

    it('closes the short link once the sign-in finished', async () => {
        const shown: string[] = [];
        const target = new Authorizer({ openUrl: opener('accepted'), askForRedirectUrl: async url => {
            shown.push(url);
            return { status: 'pasted', url: landedOn(await followLink(url)) };
        } });
        await target.authorize(pastedAuth());
        await new Promise(resolve => setTimeout(resolve, 20));
        await expect(followLink(shown[0])).rejects.toThrow();
    });

    it('gives the agent a short link to the sign-in page', async () => {
        const first = await withOpener(opener('accepted')).authorize(pastedAuth());
        expect(first.status === 'authorization_required' && first.authorizationUrl).toMatch(/^http:\/\/127\.0\.0\.1:\d+\/[0-9a-f]{32}$/);
    });

    it('asks the agent for the address when the client has no dialog', async () => {
        const first = await withOpener(opener('accepted')).authorize(pastedAuth());
        const authorizationUrl = first.status === 'authorization_required' ? await followLink(first.authorizationUrl) : '';
        expect([first.status === 'authorization_required' && first.redirectUrlRequired, new URL(authorizationUrl).searchParams.get('redirect_uri')]).toEqual([true, PASTED_REDIRECT_URI]);
    });

    it('returns the token once the agent passes the address the user pasted', async () => {
        expect(await pastedInChat(withOpener(opener('accepted')), pastedAuth())).toEqual({ status: 'authorized', token: 'granted-token' });
    });

    it('rejects an address from the agent that is not the redirect uri and keeps waiting for the right one', async () => {
        const target = withOpener(opener('accepted'));
        const first = await target.authorize(pastedAuth());
        const url = first.status === 'authorization_required' ? await followLink(first.authorizationUrl) : '';
        await expect(target.authorize(pastedAuth(), undefined, 'https://evil.example.com/callback?code=stolen')).rejects.toThrow('still waiting for the right address');
        expect(await target.authorize(pastedAuth(), undefined, landedOn(url))).toEqual({ status: 'authorized', token: 'granted-token' });
    });

    it('rejects an address whose state belongs to another sign-in', async () => {
        const target = withOpener(opener('accepted'));
        await expect(pastedInChat(target, pastedAuth(), () => `${PASTED_REDIRECT_URI}?code=granted&state=forged`)).rejects.toThrow('comes from another sign-in');
    });

    it('refuses an address from the agent for an auth with neither PKCE nor state', async () => {
        const target = withOpener(opener('accepted'));
        await expect(target.authorize(pastedAuth({ use_pkce: 'false', use_state: 'false' }))).rejects.toThrow('turns off both PKCE and state');
    });

    it('refuses the implicit flow when the client has no dialog, so the token never reaches the agent', async () => {
        const target = withOpener(opener('accepted'));
        await expect(target.authorize(pastedAuth({}, 'oauth2_implicit'))).rejects.toThrow('implicit flow');
    });

    it('explains that no sign-in waits for an address passed when none was asked for', async () => {
        const target = withOpener(opener('accepted'));
        await expect(target.authorize(pastedAuth(), undefined, `${PASTED_REDIRECT_URI}?code=old`)).rejects.toThrow('no sign-in to login is waiting for an address any more');
    });

    it('starts no sign-in for an address passed when none was asked for', async () => {
        const open = opener('accepted');
        await withOpener(open).authorize(pastedAuth(), undefined, `${PASTED_REDIRECT_URI}?code=old`).catch(() => undefined);
        expect([open.mock.calls.length, fetchMock.jestMock.mock.calls.length]).toEqual([0, 0]);
    });

    it('starts a new sign-in once nobody passed the address in time', async () => {
        const target = new Authorizer({ openUrl: opener('accepted'), askForRedirectUrl: async () => ({ status: 'unavailable' }) }, undefined, 10, 10);
        const first = await target.authorize(pastedAuth());
        await new Promise(resolve => setTimeout(resolve, 30));
        const second = await target.authorize(pastedAuth());
        expect(second.status === 'authorization_required' && first.status === 'authorization_required' && second.authorizationUrl !== first.authorizationUrl).toBe(true);
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
