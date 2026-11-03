import { isLoopbackRedirect } from '../auth/callbackHandlers';
import { OAuthProvider } from '../auth/oauthProvider';
import { performOAuth2Flow } from '../auth/oauthService';
import { AuthUi, OAuthAuthConfig, TokenStore } from '../auth/types';
import { CANCELLED_MESSAGE } from '../utils';
import { surface } from './surface';

export type AuthorizationUrlOutcome = 'accepted' | 'declined' | 'unavailable';

export type OpenAuthorizationUrl = (url: string, auth: OAuthAuthConfig, finished: Promise<void>, signal?: AbortSignal) => Promise<AuthorizationUrlOutcome>;

export type Authorization =
    | { status: 'authorized'; token: string }
    | { status: 'authorization_required'; authorizationUrl: string }
    | { status: 'declined' };

export type Authorize = (auth: OAuthAuthConfig, signal?: AbortSignal) => Promise<Authorization>;

interface PendingAuthorization {
    token: Promise<string>;
    published: Promise<string>;
}

const RETRY_GRACE_MS = 5000;
const INTERACTIVE_AUTH_TYPES = new Set(['oauth2_authorization_code', 'oauth2_implicit']);

class AuthorizationDeclinedError extends Error {}

export function isInteractiveAuth(authType: string): boolean {
    return INTERACTIVE_AUTH_TYPES.has(authType);
}

export function memoryTokenStore(): TokenStore {
    let value: string | undefined;
    return {
        read: async () => value,
        write: async next => { value = next; },
        clear: async () => { value = undefined; }
    };
}

export class Authorizer {
    private readonly store = memoryTokenStore();
    private readonly pending = new Map<string, PendingAuthorization>();

    constructor(
        private readonly openUrl: OpenAuthorizationUrl,
        private readonly log: (message: string) => void = () => undefined,
        private readonly retryGraceMs = RETRY_GRACE_MS
    ) {}

    async authorize(auth: OAuthAuthConfig, signal?: AbortSignal): Promise<Authorization> {
        requireReachableRedirect(auth);
        const key = JSON.stringify([auth.name, auth.environment ?? null]);
        const existing = this.pending.get(key);
        const pending = existing ?? this.start(key, auth, signal);
        const published = existing ? delay(this.retryGraceMs).then(() => pending.published) : pending.published;
        return Promise.race<Authorization>([
            pending.token.then(
                token => ({ status: 'authorized', token }),
                error => {
                    if (error instanceof AuthorizationDeclinedError) {
                        return { status: 'declined' };
                    }
                    throw error;
                }
            ),
            published.then(authorizationUrl => ({ status: 'authorization_required', authorizationUrl })),
            cancellation(signal)
        ]);
    }

    private start(key: string, auth: OAuthAuthConfig, signal?: AbortSignal): PendingAuthorization {
        let publish: (url: string) => void = () => undefined;
        const published = new Promise<string>(resolve => { publish = resolve; });
        let finish: () => void = () => undefined;
        const finished = new Promise<void>(resolve => { finish = resolve; });
        const ui: AuthUi = {
            openUrl: async url => {
                const outcome = await this.openUrl(url, auth, finished, signal);
                if (outcome === 'declined') {
                    throw new AuthorizationDeclinedError(`The user declined to authorize ${auth.name}`);
                }
                if (outcome === 'unavailable') {
                    publish(url);
                }
            },
            askForRedirectUrl: async () => { throw new Error(unsupportedRedirect(auth)); },
            showError: message => this.log(message),
            log: message => this.log(message)
        };
        const token = performOAuth2Flow(auth, new OAuthProvider(this.store, ui)).finally(() => {
            this.pending.delete(key);
            finish();
        });
        token.catch(() => undefined);
        const pending = { token, published };
        this.pending.set(key, pending);
        return pending;
    }
}

function requireReachableRedirect(auth: OAuthAuthConfig): void {
    const redirectUri = auth.fields.redirect_uri;
    if (auth.auth_type !== 'oauth2_authorization_code' || !redirectUri || !isLoopbackRedirect(redirectUri)) {
        throw new Error(unsupportedRedirect(auth));
    }
}

function unsupportedRedirect(auth: OAuthAuthConfig): string {
    const template = auth.auth_type === 'oauth2_implicit' ? surface.run_request.implicit_unsupported : surface.run_request.unsupported_redirect;
    return template
        .replace('{auth}', auth.name)
        .replace('{redirect_uri}', auth.fields.redirect_uri || 'the VS Code default');
}

function delay(ms: number): Promise<void> {
    return new Promise(resolve => setTimeout(resolve, ms).unref());
}

function cancellation(signal?: AbortSignal): Promise<never> {
    return new Promise((_, reject) => {
        if (signal?.aborted) {
            reject(new Error(CANCELLED_MESSAGE));
            return;
        }
        signal?.addEventListener('abort', () => reject(new Error(CANCELLED_MESSAGE)), { once: true });
    });
}
