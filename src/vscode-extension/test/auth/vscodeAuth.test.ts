import * as vscode from 'vscode';
import { secretTokenStore, urlOf, vscodeAuthUi } from '../../src/auth/vscodeAuth';
import * as authUriHandler from '../../src/auth/authUriHandler';

jest.mock('../../src/auth/authUriHandler', () => ({ waitForUri: jest.fn() }));

describe('urlOf', () => {
    test('keeps the query and the fragment of a vscode uri', () => {
        const target = urlOf(vscode.Uri.parse('vscode://rq-lang.rq-language/oauth-callback?code=foo&state=bar#access_token=baz'));
        expect([target.searchParams.get('code'), target.searchParams.get('state'), target.hash]).toEqual(['foo', 'bar', '#access_token=baz']);
    });
});

describe('vscodeAuthUi', () => {
    beforeEach(() => jest.clearAllMocks());

    test('waits for the app redirect through the uri handler', async () => {
        (authUriHandler.waitForUri as jest.Mock).mockImplementation(async (accepts: (uri: vscode.Uri) => boolean) => {
            const uri = vscode.Uri.parse('vscode://rq-lang.rq-language/oauth-callback?code=foo');
            expect(accepts(uri)).toBe(true);
            return uri;
        });
        const target = await vscodeAuthUi().waitForAppRedirect!(url => url.searchParams.has('code'));
        expect(target.searchParams.get('code')).toBe('foo');
    });

    test('stops waiting for the app redirect once the sign-in is cancelled', async () => {
        (authUriHandler.waitForUri as jest.Mock).mockResolvedValue(vscode.Uri.parse('vscode://rq-lang.rq-language/oauth-callback?code=foo'));
        const cancellation = new AbortController();
        await vscodeAuthUi(undefined, cancellation.signal).waitForAppRedirect!(() => true);
        expect((authUriHandler.waitForUri as jest.Mock).mock.calls[0][2]).toBe(cancellation.signal);
    });

    test('closes the paste box once the sign-in is cancelled', async () => {
        (vscode.window.showInformationMessage as jest.Mock).mockResolvedValue(undefined);
        (vscode.window.showInputBox as jest.Mock).mockResolvedValue(undefined);
        const cancellation = new AbortController();
        const asking = vscodeAuthUi(undefined, cancellation.signal).askForRedirectUrl({ authorizationUrl: 'https://auth', prompt: 'paste' });
        const token = (vscode.window.showInputBox as jest.Mock).mock.calls[0][1];
        cancellation.abort();
        await asking;
        expect(token.isCancellationRequested).toBe(true);
    });

    test('maps an accepted pasted url to no validation message', async () => {
        (vscode.window.showInformationMessage as jest.Mock).mockResolvedValue(undefined);
        (vscode.window.showInputBox as jest.Mock).mockResolvedValue('x');
        await vscodeAuthUi().askForRedirectUrl({ authorizationUrl: 'https://auth', prompt: 'paste', validate: () => undefined });
        expect((vscode.window.showInputBox as jest.Mock).mock.calls[0][0].validateInput('x')).toBeNull();
    });
});

describe('secretTokenStore', () => {
    test('keeps the token cache under the key earlier versions used', async () => {
        const store = jest.fn();
        const context = { secrets: { get: jest.fn(), store, delete: jest.fn() } } as unknown as vscode.ExtensionContext;
        await secretTokenStore(context).write('cache');
        expect(store).toHaveBeenCalledWith('oauth-token-cache', 'cache');
    });
});
