import * as vscode from 'vscode';
import { OAuthProvider } from './oauthProvider';
import { waitForUri } from './authUriHandler';
import { AuthUi, RedirectUrlRequest, TokenStore } from './types';

const TOKEN_CACHE_KEY = 'oauth-token-cache';
const OPEN_BROWSER_AGAIN = 'Open Browser Again';

export function vscodeAuthUi(outputChannel?: vscode.OutputChannel): AuthUi {
    return {
        openUrl: async url => { await vscode.env.openExternal(vscode.Uri.parse(url)); },
        askForRedirectUrl: request => askForRedirectUrl(request),
        waitForAppRedirect: async accepts => {
            const uri = await waitForUri(candidate => accepts(urlOf(candidate)));
            return urlOf(uri);
        },
        showError: message => { vscode.window.showErrorMessage(message); },
        log: message => {
            console.log(message);
            outputChannel?.appendLine(`[OAuth] ${message}`);
        }
    };
}

export function secretTokenStore(context: vscode.ExtensionContext): TokenStore {
    return {
        read: async () => context.secrets.get(TOKEN_CACHE_KEY),
        write: async value => { await context.secrets.store(TOKEN_CACHE_KEY, value); },
        clear: async () => { await context.secrets.delete(TOKEN_CACHE_KEY); }
    };
}

export function vscodeOAuthProvider(context: vscode.ExtensionContext, outputChannel?: vscode.OutputChannel): OAuthProvider {
    return new OAuthProvider(secretTokenStore(context), vscodeAuthUi(outputChannel));
}

export function urlOf(uri: vscode.Uri): URL {
    const url = new URL(`${uri.scheme}://${uri.authority}${uri.path}`);
    url.search = uri.query;
    url.hash = uri.fragment;
    return url;
}

async function askForRedirectUrl(request: RedirectUrlRequest): Promise<string | undefined> {
    vscode.window.showInformationMessage(
        'Complete the authentication in your browser, then paste the callback URL here.',
        OPEN_BROWSER_AGAIN
    ).then(action => {
        if (action === OPEN_BROWSER_AGAIN) {
            vscode.env.openExternal(vscode.Uri.parse(request.authorizationUrl));
        }
    });
    return vscode.window.showInputBox({
        prompt: request.prompt,
        placeHolder: request.placeholder,
        ignoreFocusOut: true,
        validateInput: request.validate ? value => request.validate?.(value) ?? null : undefined
    });
}
