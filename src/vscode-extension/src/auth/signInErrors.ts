export const SIGN_IN_TIMEOUT_MS = 5 * 60 * 1000;

export function verifyState(expected: string | undefined, received: string | null): void {
    if (!received) {
        throw new Error(
            'The address the sign-in ended on has no state, so rq did not use it: without one rq cannot tell that address comes from this sign-in. ' +
            'The sign-in page was most likely opened from an incomplete link, cut off before its state. ' +
            'Sign in again, opening the whole sign-in link, and use the address the browser ends on.'
        );
    }
    if (received !== expected) {
        throw new Error(
            'The address the sign-in ended on comes from another sign-in (its state does not match this one), so rq did not use it. ' +
            'Use the address the browser ends on after the sign-in started for this request.'
        );
    }
}

export function providerRefusedError(error: string, description: string | null): Error {
    return new Error(`The authorization server refused the sign-in: ${error}${description ? ` (${description})` : ''}.`);
}

export function missingCodeError(): Error {
    return new Error(
        'The address the sign-in ended on has no authorization code, so there is nothing to exchange for a token. ' +
        'Use the whole address the browser ends on after signing in, including everything after "?".'
    );
}

export function missingAccessTokenError(): Error {
    return new Error(
        'The address the sign-in ended on has no access_token. ' +
        'Use the whole address the browser ends on after signing in, including everything after "#".'
    );
}

export function notAnAddressError(redirectUri?: string): Error {
    const startsWith = redirectUri ? `, which starts with ${redirectUri}` : '';
    return new Error(`The pasted text is not an address. Paste the whole address the browser ends on after signing in${startsWith}.`);
}

export function wrongRedirectMessage(redirectUri: string): string {
    return `The pasted address does not start with the redirect_uri ${redirectUri}, so it is not where this sign-in ends. Paste the whole address the browser ends on after signing in.`;
}

export function tokenExchangeError(tokenUrl: string, status: number, detail: string): Error {
    return new Error(
        `The authorization server rejected the authorization code at ${tokenUrl} (HTTP ${status}: ${detail}). ` +
        'Check the client_id, client_secret, token_url and redirect_uri of the auth. A code also stops working once used or after a short time, so sign in again.'
    );
}

export function signInTimeoutError(): Error {
    return new Error('No sign-in finished within 5 minutes, so rq stopped waiting. Run the request again to start a new sign-in.');
}

export function callbackListenError(redirectUri: string, port: number, err: NodeJS.ErrnoException): Error {
    if (err.code === 'EADDRINUSE') {
        return new Error(
            `Port ${port} of the redirect_uri ${redirectUri} is in use by another program, so rq cannot receive the sign-in there. ` +
            'Close that program, or register a redirect_uri with a free port for the app and use it.'
        );
    }
    return new Error(`rq cannot listen on ${redirectUri} to receive the sign-in: ${err.message}.`);
}
