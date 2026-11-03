import { isAppRedirect, isLoopbackRedirect } from '../auth/callbackHandlers';
import { OAuthProvider, tokenCacheKey } from '../auth/oauthProvider';
import { performOAuth2Flow } from '../auth/oauthService';
import { signInTimeoutError } from '../auth/signInErrors';
import { AuthUi, OAuthAuthConfig, RedirectUrlRequest, TokenStore } from '../auth/types';
import { CANCELLED_MESSAGE } from '../utils';
import { openShortLink, ShortLink } from './shortLink';
import { surface } from './surface';

export type AuthorizationUrlOutcome = 'accepted' | 'declined' | 'unavailable';

export type RedirectUrlOutcome = { status: 'pasted'; url: string } | { status: 'declined' } | { status: 'unavailable' };

export interface AuthorizationPrompts {
    openUrl(url: string, auth: OAuthAuthConfig, finished: Promise<void>, signal?: AbortSignal): Promise<AuthorizationUrlOutcome>;
    askForRedirectUrl(authorizationUrl: string, auth: OAuthAuthConfig, signal?: AbortSignal): Promise<RedirectUrlOutcome>;
}

export type Authorization =
    | { status: 'authorized'; token: string }
    | { status: 'authorization_required'; authorizationUrl: string; redirectUrlRequired: boolean }
    | { status: 'declined' };

export type Authorize = (auth: OAuthAuthConfig, signal?: AbortSignal, redirectUrl?: string) => Promise<Authorization>;

interface PublishedAuthorization {
    url: string;
    redirectUrlRequired: boolean;
}

interface AwaitedRedirectUrl {
    validate?: (value: string) => string | undefined;
    resolve: (url: string) => void;
}

interface PendingAuthorization {
    token: Promise<string>;
    published: Promise<PublishedAuthorization>;
    awaitedRedirectUrl?: AwaitedRedirectUrl;
}

const RETRY_GRACE_MS = 5000;
const REDIRECT_URL_TIMEOUT_MS = 5 * 60 * 1000;
const INTERACTIVE_AUTH_TYPES = new Set(['oauth2_authorization_code', 'oauth2_implicit']);

class AuthorizationDeclinedError extends Error {}

export class McpAuthorizationError extends Error {}

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
        private readonly prompts: AuthorizationPrompts,
        private readonly log: (message: string) => void = () => undefined,
        private readonly retryGraceMs = RETRY_GRACE_MS,
        private readonly redirectUrlTimeoutMs = REDIRECT_URL_TIMEOUT_MS
    ) {}

    async authorize(auth: OAuthAuthConfig, signal?: AbortSignal, redirectUrl?: string): Promise<Authorization> {
        requireSupportedRedirect(auth);
        const key = tokenCacheKey({ authName: auth.name, environment: auth.environment, file: auth.file });
        const existing = this.pending.get(key);
        if (redirectUrl && existing?.awaitedRedirectUrl) {
            supplyRedirectUrl(existing.awaitedRedirectUrl, redirectUrl);
            existing.awaitedRedirectUrl = undefined;
            return Promise.race([outcomeOf(existing.token), cancellation(signal)]);
        }
        const pending = existing ?? this.start(key, auth, signal);
        const published = existing ? delay(this.retryGraceMs).then(() => pending.published) : pending.published;
        return Promise.race<Authorization>([
            outcomeOf(pending.token),
            published.then(({ url, redirectUrlRequired }) => ({ status: 'authorization_required', authorizationUrl: url, redirectUrlRequired })),
            cancellation(signal)
        ]);
    }

    private start(key: string, auth: OAuthAuthConfig, signal?: AbortSignal): PendingAuthorization {
        let publish: (published: PublishedAuthorization) => void = () => undefined;
        const published = new Promise<PublishedAuthorization>(resolve => { publish = resolve; });
        let finish: () => void = () => undefined;
        const finished = new Promise<void>(resolve => { finish = resolve; });
        const pending: PendingAuthorization = { token: new Promise<string>(() => undefined), published };
        let link: Promise<ShortLink> | undefined;
        const shortLink = async (url: string) => {
            link ??= openShortLink(url, this.redirectUrlTimeoutMs);
            return (await link).url;
        };
        const ui: AuthUi = {
            openUrl: async url => {
                if (!receivesRedirect(auth)) {
                    return;
                }
                const outcome = await this.prompts.openUrl(url, auth, finished, signal);
                if (outcome === 'declined') {
                    throw new AuthorizationDeclinedError(`The user declined to authorize ${auth.name}`);
                }
                if (outcome === 'unavailable') {
                    publish({ url: await shortLink(url), redirectUrlRequired: false });
                }
            },
            askForRedirectUrl: async request => {
                const outcome = await this.prompts.askForRedirectUrl(await shortLink(request.authorizationUrl), auth, signal);
                if (outcome.status === 'declined') {
                    throw new AuthorizationDeclinedError(`The user declined to authorize ${auth.name}`);
                }
                if (outcome.status === 'pasted') {
                    const invalid = request.validate?.(outcome.url);
                    if (invalid) {
                        throw new Error(invalid);
                    }
                    return outcome.url;
                }
                if (!acceptsRedirectUrlFromAgent(auth)) {
                    throw new McpAuthorizationError(redirectUrlNeedsDialog(auth));
                }
                const pasted = this.awaitRedirectUrl(pending, request);
                publish({ url: await shortLink(request.authorizationUrl), redirectUrlRequired: true });
                return pasted;
            },
            showError: message => this.log(message),
            log: message => this.log(message)
        };
        pending.token = performOAuth2Flow(auth, new OAuthProvider(this.store, ui)).finally(() => {
            this.pending.delete(key);
            link?.then(opened => opened.close(), () => undefined);
            finish();
        });
        pending.token.catch(() => undefined);
        this.pending.set(key, pending);
        return pending;
    }

    private awaitRedirectUrl(pending: PendingAuthorization, request: RedirectUrlRequest): Promise<string> {
        return new Promise((resolve, reject) => {
            const timer = setTimeout(() => {
                pending.awaitedRedirectUrl = undefined;
                reject(signInTimeoutError());
            }, this.redirectUrlTimeoutMs);
            timer.unref();
            pending.awaitedRedirectUrl = {
                validate: request.validate,
                resolve: url => {
                    clearTimeout(timer);
                    resolve(url);
                }
            };
        });
    }
}

function supplyRedirectUrl(awaited: AwaitedRedirectUrl, redirectUrl: string): void {
    const invalid = awaited.validate?.(redirectUrl);
    if (invalid) {
        throw new McpAuthorizationError(surface.run_request.invalid_redirect_url.replace('{reason}', invalid));
    }
    awaited.resolve(redirectUrl);
}

function outcomeOf(token: Promise<string>): Promise<Authorization> {
    return token.then(
        accessToken => ({ status: 'authorized', token: accessToken }),
        error => {
            if (error instanceof AuthorizationDeclinedError) {
                return { status: 'declined' };
            }
            throw error;
        }
    );
}

function receivesRedirect(auth: OAuthAuthConfig): boolean {
    return auth.auth_type === 'oauth2_authorization_code' && isLoopbackRedirect(auth.fields.redirect_uri ?? '');
}

function acceptsRedirectUrlFromAgent(auth: OAuthAuthConfig): boolean {
    return auth.auth_type === 'oauth2_authorization_code' && !(auth.fields.use_pkce === 'false' && auth.fields.use_state === 'false');
}

function requireSupportedRedirect(auth: OAuthAuthConfig): void {
    const redirectUri = auth.fields.redirect_uri;
    if (!redirectUri || isAppRedirect(redirectUri)) {
        throw new McpAuthorizationError(surface.run_request.unsupported_redirect
            .replace('{auth}', auth.name)
            .replace('{redirect_uri}', redirectUri || 'the VS Code default'));
    }
}

function redirectUrlNeedsDialog(auth: OAuthAuthConfig): string {
    const template = auth.auth_type === 'oauth2_implicit' ? surface.run_request.implicit_needs_dialog : surface.run_request.unprotected_needs_dialog;
    return template.replace('{auth}', auth.name);
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
