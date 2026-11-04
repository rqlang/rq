import { URL, URLSearchParams } from 'url';
import * as http from 'http';
import { AuthUi } from './types';
import { CANCELLED_MESSAGE } from '../utils';
import { callbackListenError, missingAccessTokenError, missingCodeError, notAnAddressError, providerRefusedError, SIGN_IN_TIMEOUT_MS, signInTimeoutError, wrongRedirectMessage } from './signInErrors';

export interface CallbackResult {
    code: string;
    state: string | null;
}

export interface ImplicitCallbackResult {
    accessToken: string;
    tokenType?: string;
    expiresIn?: number;
    state: string | null;
}

const DEFAULT_CALLBACK_PORT = 3000;
const MIN_PASTED_TOKEN_LENGTH = 20;
const LOOPBACK_HOSTS = new Set(['localhost', '127.0.0.1']);

export function isAppRedirect(redirectUri: string): boolean {
    const lower = redirectUri.toLowerCase();
    return lower.startsWith('vscode://') || lower.startsWith('vscode-insiders://');
}

export function isLoopbackRedirect(redirectUri: string): boolean {
    try {
        const url = new URL(redirectUri);
        return url.protocol === 'http:' && LOOPBACK_HOSTS.has(url.hostname);
    } catch {
        return false;
    }
}

export async function handleAppRedirectCallback(ui: AuthUi, authUrl: string, redirectUri: string): Promise<CallbackResult> {
    const waitForAppRedirect = appRedirectWaiter(ui, redirectUri);
    await ui.openUrl(authUrl);
    const url = await waitForAppRedirect(candidate => candidate.searchParams.has('code') || candidate.searchParams.has('error'));
    ui.log('Received the authorization redirect');
    return codeFrom(ui, url.searchParams);
}

export async function handleAppRedirectImplicitCallback(ui: AuthUi, authUrl: string, redirectUri: string): Promise<ImplicitCallbackResult> {
    const waitForAppRedirect = appRedirectWaiter(ui, redirectUri);
    await ui.openUrl(authUrl);
    const url = await waitForAppRedirect(candidate => {
        const params = implicitParams(candidate);
        return params('access_token') !== null || params('error') !== null;
    });
    ui.log('Received the authorization redirect (implicit)');
    return implicitResultFrom(url);
}

export async function handleLocalServerCallback(ui: AuthUi, authUrl: string, redirectUri: string): Promise<CallbackResult> {
    const redirectUrl = new URL(redirectUri);
    return new Promise((resolve, reject) => {
        let timer: ReturnType<typeof setTimeout> | undefined;
        let finished = false;
        const cancel = () => finish(() => reject(new Error(CANCELLED_MESSAGE)));
        const finish = (outcome: () => void) => {
            if (finished) {
                return;
            }
            finished = true;
            clearTimeout(timer);
            ui.signal?.removeEventListener('abort', cancel);
            server.close();
            outcome();
        };
        const server = http.createServer((req, res) => {
            const url = new URL(req.url || '', redirectUri);
            if (url.pathname !== redirectUrl.pathname) {
                res.writeHead(404, { 'Connection': 'close' });
                res.end();
                return;
            }
            const code = url.searchParams.get('code');
            const error = url.searchParams.get('error');
            if (error) {
                const description = url.searchParams.get('error_description');
                respond(res, 400, 'Authentication Error', `<p><strong>Error:</strong> ${escapeHtml(error)}</p><p>${escapeHtml(description || '')}</p>`);
                finish(() => reject(providerRefusedError(error, description)));
                return;
            }
            if (code) {
                respond(res, 200, 'Authentication Successful!', '<p>You have been authenticated successfully.</p>');
                ui.log('Authorization code received via local server');
                finish(() => resolve({ code, state: url.searchParams.get('state') }));
                return;
            }
            respond(res, 400, 'Invalid Response', '<p>No authorization code received.</p>');
            finish(() => reject(missingCodeError()));
        });
        if (ui.signal?.aborted) {
            reject(new Error(CANCELLED_MESSAGE));
            return;
        }
        ui.signal?.addEventListener('abort', cancel, { once: true });
        const port = parseInt(redirectUrl.port) || DEFAULT_CALLBACK_PORT;
        server.on('error', err => finish(() => reject(callbackListenError(redirectUri, port, err))));
        server.listen(port, redirectUrl.hostname, () => {
            ui.log(`Local callback server started on ${redirectUri}`);
            ui.openUrl(authUrl).catch(err => finish(() => reject(err)));
        });
        timer = setTimeout(() => {
            finish(() => reject(signInTimeoutError()));
        }, SIGN_IN_TIMEOUT_MS);
    });
}

export async function handleManualPasteCallback(ui: AuthUi, authUrl: string, redirectUri: string): Promise<CallbackResult> {
    await ui.openUrl(authUrl);
    const callbackUrl = await ui.askForRedirectUrl({
        authorizationUrl: authUrl,
        prompt: 'Paste the full callback URL from your browser',
        placeholder: `${redirectUri}?code=...`,
        validate: value => {
            if (!value) {
                return 'Please paste the callback URL';
            }
            return value.startsWith(redirectUri) ? undefined : wrongRedirectMessage(redirectUri);
        }
    });
    if (!callbackUrl) {
        throw new Error('Authentication cancelled - no callback URL provided');
    }
    ui.log('Authorization code received via manual paste');
    return codeFrom(ui, parsedAddress(callbackUrl, redirectUri).searchParams);
}

export async function handleManualPasteImplicitCallback(ui: AuthUi, authUrl: string): Promise<ImplicitCallbackResult> {
    await ui.openUrl(authUrl);
    const callbackUrl = await ui.askForRedirectUrl({
        authorizationUrl: authUrl,
        prompt: 'Authorize in the browser, then paste the FULL result URL here (including #access_token=...)'
    });
    if (!callbackUrl) {
        throw new Error('Authentication cancelled');
    }
    let url: URL;
    try {
        url = new URL(callbackUrl);
    } catch {
        if (callbackUrl.length > MIN_PASTED_TOKEN_LENGTH && !callbackUrl.includes(' ')) {
            return { accessToken: callbackUrl, tokenType: 'Bearer', state: null };
        }
        throw new Error('The pasted text is neither an address nor an access token. Paste the whole address the browser ends on after signing in.');
    }
    return implicitResultFrom(url);
}

function appRedirectWaiter(ui: AuthUi, redirectUri: string): (accepts: (url: URL) => boolean) => Promise<URL> {
    if (!ui.waitForAppRedirect) {
        throw new Error(`The redirect_uri ${redirectUri} only works inside VS Code. Register an http://localhost:PORT redirect for the app and use it as redirect_uri.`);
    }
    return accepts => ui.waitForAppRedirect!(accepts);
}

function codeFrom(ui: AuthUi, params: URLSearchParams): CallbackResult {
    const error = params.get('error');
    if (error) {
        const refused = providerRefusedError(error, params.get('error_description'));
        ui.showError(refused.message);
        throw refused;
    }
    const code = params.get('code');
    if (!code) {
        throw missingCodeError();
    }
    return { code, state: params.get('state') };
}

function implicitResultFrom(url: URL): ImplicitCallbackResult {
    const params = implicitParams(url);
    const error = params('error');
    if (error) {
        throw providerRefusedError(error, params('error_description'));
    }
    const accessToken = params('access_token');
    if (!accessToken) {
        throw missingAccessTokenError();
    }
    const expiresIn = params('expires_in');
    return {
        accessToken,
        tokenType: params('token_type') || 'Bearer',
        expiresIn: expiresIn ? parseInt(expiresIn, 10) : undefined,
        state: params('state')
    };
}

function parsedAddress(text: string, redirectUri: string): URL {
    try {
        return new URL(text);
    } catch {
        throw notAnAddressError(redirectUri);
    }
}

function implicitParams(url: URL): (key: string) => string | null {
    const fragment = new URLSearchParams(url.hash.replace(/^#/, ''));
    return key => fragment.get(key) || url.searchParams.get(key);
}

function respond(res: http.ServerResponse, status: number, title: string, body: string): void {
    const color = status === 200 ? '#0a0' : '#c00';
    res.writeHead(status, { 'Content-Type': 'text/html', 'Connection': 'close' });
    res.end(`<html><body style="font-family: Arial, sans-serif; padding: 40px; text-align: center;"><h1 style="color: ${color};">${title}</h1>${body}<p>You can close this window.</p></body></html>`);
}

function escapeHtml(text: string): string {
    return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}
