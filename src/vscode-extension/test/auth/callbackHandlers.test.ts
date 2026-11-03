import * as http from 'http';
import {
    handleAppRedirectCallback,
    handleAppRedirectImplicitCallback,
    handleLocalServerCallback,
    handleManualPasteCallback,
    handleManualPasteImplicitCallback,
    isLoopbackRedirect
} from '../../src/auth/callbackHandlers';
import { AuthUi } from '../../src/auth/types';

const AUTH_URL = 'https://auth.example.com/authorize';
const APP_REDIRECT = 'vscode://rq-lang.rq-language/callback';

function fakeUi(overrides: Partial<AuthUi> = {}): AuthUi {
    return {
        openUrl: jest.fn().mockResolvedValue(undefined),
        askForRedirectUrl: jest.fn(),
        showError: jest.fn(),
        log: jest.fn(),
        ...overrides
    };
}

function redirectingTo(target: string): AuthUi['waitForAppRedirect'] {
    return jest.fn(async (accepts: (url: URL) => boolean) => {
        const url = new URL(target);
        expect(accepts(url)).toBe(true);
        return url;
    });
}

describe('handleAppRedirectImplicitCallback', () => {
    it('parses the access token from the fragment', async () => {
        const target = await handleAppRedirectImplicitCallback(fakeUi({ waitForAppRedirect: redirectingTo(`${APP_REDIRECT}#access_token=foo&state=bar`) }), AUTH_URL, APP_REDIRECT);
        expect(target).toMatchObject({ accessToken: 'foo', state: 'bar' });
    });

    it('parses the access token from the query when the fragment has none', async () => {
        const target = await handleAppRedirectImplicitCallback(fakeUi({ waitForAppRedirect: redirectingTo(`${APP_REDIRECT}?access_token=foo&state=bar`) }), AUTH_URL, APP_REDIRECT);
        expect(target).toMatchObject({ accessToken: 'foo', state: 'bar' });
    });

    it('opens the authorization url', async () => {
        const ui = fakeUi({ waitForAppRedirect: redirectingTo(`${APP_REDIRECT}#access_token=foo`) });
        await handleAppRedirectImplicitCallback(ui, AUTH_URL, APP_REDIRECT);
        expect(ui.openUrl).toHaveBeenCalledWith(AUTH_URL);
    });

    it('reports an error in the fragment', async () => {
        const ui = fakeUi({ waitForAppRedirect: redirectingTo(`${APP_REDIRECT}#error=access_denied&error_description=bad`) });
        await expect(handleAppRedirectImplicitCallback(ui, AUTH_URL, APP_REDIRECT)).rejects.toThrow('The authorization server refused the sign-in: access_denied (bad).');
    });
});

describe('handleAppRedirectCallback', () => {
    it('parses the code from the query', async () => {
        const target = await handleAppRedirectCallback(fakeUi({ waitForAppRedirect: redirectingTo(`${APP_REDIRECT}?code=foo&state=bar`) }), AUTH_URL, APP_REDIRECT);
        expect(target).toEqual({ code: 'foo', state: 'bar' });
    });

    it('reports an error in the query', async () => {
        const ui = fakeUi({ waitForAppRedirect: redirectingTo(`${APP_REDIRECT}?error=invalid_request`) });
        await expect(handleAppRedirectCallback(ui, AUTH_URL, APP_REDIRECT)).rejects.toThrow(/refused the sign-in: invalid_request\.$/);
    });

    it('refuses an app redirect when the ui cannot receive one, before opening the browser', async () => {
        const ui = fakeUi();
        await expect(handleAppRedirectCallback(ui, AUTH_URL, APP_REDIRECT)).rejects.toThrow('only works inside VS Code');
        expect(ui.openUrl).not.toHaveBeenCalled();
    });
});

describe('handleManualPasteCallback', () => {
    const redirectUri = 'https://app.example.com/callback';

    it('reads the code from the pasted url', async () => {
        const ui = fakeUi({ askForRedirectUrl: jest.fn().mockResolvedValue(`${redirectUri}?code=foo&state=bar`) });
        expect(await handleManualPasteCallback(ui, AUTH_URL, redirectUri)).toEqual({ code: 'foo', state: 'bar' });
    });

    it('rejects a pasted url that does not start with the redirect uri', async () => {
        const askForRedirectUrl = jest.fn().mockResolvedValue(`${redirectUri}?code=foo`);
        await handleManualPasteCallback(fakeUi({ askForRedirectUrl }), AUTH_URL, redirectUri);
        expect(askForRedirectUrl.mock.calls[0][0].validate('https://evil.example.com/?code=foo')).toBe(`The pasted address does not start with the redirect_uri ${redirectUri}, so it is not where this sign-in ends. Paste the whole address the browser ends on after signing in.`);
    });

    it('explains that pasted text which is not an address cannot be used', async () => {
        const ui = fakeUi({ askForRedirectUrl: jest.fn().mockResolvedValue('just some text') });
        await expect(handleManualPasteCallback(ui, AUTH_URL, redirectUri)).rejects.toThrow(`The pasted text is not an address. Paste the whole address the browser ends on after signing in, which starts with ${redirectUri}.`);
    });

    it('explains that a pasted address without a code cannot be used', async () => {
        const ui = fakeUi({ askForRedirectUrl: jest.fn().mockResolvedValue(`${redirectUri}?state=bar`) });
        await expect(handleManualPasteCallback(ui, AUTH_URL, redirectUri)).rejects.toThrow('has no authorization code');
    });

    it('reports a cancelled paste', async () => {
        const ui = fakeUi({ askForRedirectUrl: jest.fn().mockResolvedValue(undefined) });
        await expect(handleManualPasteCallback(ui, AUTH_URL, redirectUri)).rejects.toThrow('Authentication cancelled');
    });
});

describe('handleManualPasteImplicitCallback', () => {
    it('opens the authorization url before asking for the pasted one', async () => {
        const ui = fakeUi({ askForRedirectUrl: jest.fn().mockResolvedValue('https://app.example.com/cb#access_token=foo&state=bar') });
        await handleManualPasteImplicitCallback(ui, AUTH_URL);
        expect([(ui.openUrl as jest.Mock).mock.calls[0][0], (ui.openUrl as jest.Mock).mock.invocationCallOrder[0] < (ui.askForRedirectUrl as jest.Mock).mock.invocationCallOrder[0]]).toEqual([AUTH_URL, true]);
    });

    it('reads the access token from the pasted url', async () => {
        const ui = fakeUi({ askForRedirectUrl: jest.fn().mockResolvedValue('https://app.example.com/cb#access_token=foo&state=bar') });
        expect(await handleManualPasteImplicitCallback(ui, AUTH_URL)).toMatchObject({ accessToken: 'foo', state: 'bar' });
    });

    it('accepts a pasted bare token', async () => {
        const ui = fakeUi({ askForRedirectUrl: jest.fn().mockResolvedValue('eyJhbGciOiJIUzI1NiJ9.payload.signature') });
        expect(await handleManualPasteImplicitCallback(ui, AUTH_URL)).toMatchObject({ accessToken: 'eyJhbGciOiJIUzI1NiJ9.payload.signature', state: null });
    });
});

describe('isLoopbackRedirect', () => {
    it.each(['http://localhost:3000/callback', 'http://127.0.0.1:47391/cb', 'http://LOCALHOST/cb'])('accepts %s', redirectUri => {
        expect(isLoopbackRedirect(redirectUri)).toBe(true);
    });

    it.each(['http://localhost.example.com/cb', 'http://127.0.0.1.example.com/cb', 'https://localhost/cb', 'not an address'])('rejects %s', redirectUri => {
        expect(isLoopbackRedirect(redirectUri)).toBe(false);
    });
});

describe('handleLocalServerCallback', () => {
    const redirectUri = 'http://127.0.0.1:47391/callback';

    function visit(url: string): Promise<{ status: number; body: string }> {
        return new Promise((resolve, reject) => {
            http.get(url, res => {
                let body = '';
                res.on('data', chunk => { body += chunk; });
                res.on('end', () => resolve({ status: res.statusCode ?? 0, body }));
            }).on('error', reject);
        });
    }

    it('receives the code on the redirect uri and opens the authorization url once listening', async () => {
        const ui = fakeUi({ openUrl: jest.fn(async () => { await visit(`${redirectUri}?code=foo&state=bar`); }) });
        const target = await handleLocalServerCallback(ui, AUTH_URL, redirectUri);
        expect([target, (ui.openUrl as jest.Mock).mock.calls[0][0]]).toEqual([{ code: 'foo', state: 'bar' }, AUTH_URL]);
    });

    it('keeps the code it received when opening the url fails afterwards', async () => {
        const ui = fakeUi({ openUrl: jest.fn(async () => {
            await visit(`${redirectUri}?code=foo&state=bar`);
            throw new Error('declined late');
        }) });
        expect(await handleLocalServerCallback(ui, AUTH_URL, redirectUri)).toEqual({ code: 'foo', state: 'bar' });
    });

    it('listens only on the host of the redirect uri', async () => {
        const listen = jest.spyOn(http.Server.prototype, 'listen');
        const ui = fakeUi({ openUrl: jest.fn(async () => { await visit(`${redirectUri}?code=foo`); }) });
        await handleLocalServerCallback(ui, AUTH_URL, redirectUri);
        expect(listen.mock.calls[0].slice(0, 2)).toEqual([47391, '127.0.0.1']);
        listen.mockRestore();
    });

    it('answers a request for another path without ending the sign-in', async () => {
        let other: Promise<{ status: number; body: string }> | undefined;
        const ui = fakeUi({ openUrl: jest.fn(async () => {
            other = visit('http://127.0.0.1:47391/favicon.ico');
            await other;
            await visit(`${redirectUri}?code=foo&state=bar`);
        }) });
        const target = await handleLocalServerCallback(ui, AUTH_URL, redirectUri);
        expect([(await other!).status, target]).toEqual([404, { code: 'foo', state: 'bar' }]);
    });

    it('explains that the port of the redirect uri is taken', async () => {
        const blocker = http.createServer();
        await new Promise<void>(resolve => blocker.listen(47391, '127.0.0.1', resolve));
        try {
            await expect(handleLocalServerCallback(fakeUi(), AUTH_URL, redirectUri)).rejects.toThrow(`Port 47391 of the redirect_uri ${redirectUri} is in use by another program`);
        } finally {
            await new Promise(resolve => blocker.close(resolve));
        }
    });

    it('escapes the error it shows on the callback page', async () => {
        let page: Promise<{ status: number; body: string }> | undefined;
        const ui = fakeUi({ openUrl: jest.fn(async () => { page = visit(`${redirectUri}?error=%3Cscript%3E`); }) });
        await expect(handleLocalServerCallback(ui, AUTH_URL, redirectUri)).rejects.toThrow('refused the sign-in: <script>');
        const { body } = await page!;
        expect([body.includes('&lt;script&gt;'), body.includes('<script>')]).toEqual([true, false]);
    });
});
