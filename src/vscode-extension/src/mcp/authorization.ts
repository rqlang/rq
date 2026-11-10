import { isAppRedirect, isLoopbackRedirect } from '../auth/callbackHandlers';
import { OAuthProvider, tokenCacheKey } from '../auth/oauthProvider';
import { performOAuth2Flow } from '../auth/oauthService';
import { signInTimeoutError } from '../auth/signInErrors';
import { AuthUi, OAuthAuthConfig, RedirectUrlRequest, TokenStore } from '../auth/types';
import { CANCELLED_MESSAGE } from '../utils';
import { openShortLink, ShortLink } from './shortLink';
import { surface } from './surface';

export type AuthorizationUrlOutcome = 'accepted' | 'declined' | 'unanswered' | 'unavailable';

export type RedirectUrlOutcome = { status: 'pasted'; url: string } | { status: 'declined' } | { status: 'unanswered' } | { status: 'unavailable' };

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
    prompts: AbortController;
    prompting: boolean;
    waiters: number;
    awaitedRedirectUrl?: AwaitedRedirectUrl;
}

const RETRY_GRACE_MS = 5000;
const REDIRECT_URL_TIMEOUT_MS = 5 * 60 * 1000;
const INTERACTIVE_AUTH_TYPES = new Set(['oauth2_authorization_code', 'oauth2_implicit']);

class AuthorizationDeclinedError extends Error {}

class SignInNeededError extends Error {}

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
    private readonly failures = new Map<string, { error: unknown; expiresAt: number }>();

    constructor(
        private readonly prompts: AuthorizationPrompts,
        private readonly log: (message: string) => void = () => undefined,
        private readonly retryGraceMs = RETRY_GRACE_MS,
        private readonly redirectUrlTimeoutMs = REDIRECT_URL_TIMEOUT_MS
    ) {}

    async authorize(auth: OAuthAuthConfig, signal?: AbortSignal, redirectUrl?: string): Promise<Authorization> {
        requireSupportedRedirect(auth);
        const key = tokenCacheKey({ authName: auth.name, environment: auth.environment, file: auth.file });
        const failure = this.takeFailure(key);
        if (failure) {
            return outcomeOf(Promise.reject(failure.error));
        }
        const existing = this.pending.get(key);
        if (redirectUrl) {
            if (!existing?.awaitedRedirectUrl) {
                return this.cachedAuthorization(auth);
            }
            supplyRedirectUrl(existing.awaitedRedirectUrl, redirectUrl);
            existing.awaitedRedirectUrl = undefined;
            return this.wait(key, existing, outcomeOf(existing.token), signal);
        }
        const pending = existing ?? this.start(key, auth);
        const published = existing ? delay(this.retryGraceMs).then(() => pending.published) : pending.published;
        return this.wait(key, pending, Promise.race<Authorization>([
            outcomeOf(pending.token),
            published.then(({ url, redirectUrlRequired }) => ({ status: 'authorization_required', authorizationUrl: url, redirectUrlRequired }))
        ]), signal);
    }

    private takeFailure(key: string): { error: unknown } | undefined {
        const now = Date.now();
        for (const [failedKey, failure] of this.failures) {
            if (failure.expiresAt < now) {
                this.failures.delete(failedKey);
            }
        }
        const failure = this.failures.get(key);
        this.failures.delete(key);
        return failure;
    }

    private async cachedAuthorization(auth: OAuthAuthConfig): Promise<Authorization> {
        const needsSignIn = async (): Promise<never> => { throw new SignInNeededError(); };
        const ui: AuthUi = { openUrl: needsSignIn, askForRedirectUrl: needsSignIn, showError: message => this.log(message), log: message => this.log(message) };
        try {
            return { status: 'authorized', token: await performOAuth2Flow(auth, new OAuthProvider(this.store, ui)) };
        } catch (err) {
            if (err instanceof SignInNeededError) {
                throw new McpAuthorizationError(surface.run_request.unexpected_redirect_url.replace('{auth}', auth.name));
            }
            throw err;
        }
    }

    private async wait(key: string, pending: PendingAuthorization, outcome: Promise<Authorization>, signal?: AbortSignal): Promise<Authorization> {
        pending.waiters += 1;
        try {
            return await Promise.race([outcome, cancellation(signal)]);
        } finally {
            pending.waiters -= 1;
            if (signal?.aborted && pending.waiters === 0 && pending.prompting) {
                this.forget(key, pending);
                pending.prompts.abort();
            }
        }
    }

    private forget(key: string, pending: PendingAuthorization): void {
        if (this.pending.get(key) === pending) {
            this.pending.delete(key);
        }
    }

    private start(key: string, auth: OAuthAuthConfig): PendingAuthorization {
        let publish: (published: PublishedAuthorization) => void = () => undefined;
        const published = new Promise<PublishedAuthorization>(resolve => { publish = resolve; });
        let finish: () => void = () => undefined;
        const finished = new Promise<void>(resolve => { finish = resolve; });
        const pending: PendingAuthorization = { token: new Promise<string>(() => undefined), published, prompts: new AbortController(), prompting: false, waiters: 0 };
        const signal = pending.prompts.signal;
        const prompt = async <T>(ask: () => Promise<T>): Promise<T> => {
            pending.prompting = true;
            const outcome = await ask().finally(() => { pending.prompting = false; });
            throwIfCancelled(signal);
            return outcome;
        };
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
                const outcome = await prompt(() => this.prompts.openUrl(url, auth, finished, signal));
                if (outcome === 'declined') {
                    throw new AuthorizationDeclinedError(`The user declined to authorize ${auth.name}`);
                }
                if (outcome === 'unanswered') {
                    throw signInTimeoutError();
                }
                if (outcome === 'unavailable') {
                    publish({ url: await shortLink(url), redirectUrlRequired: false });
                }
            },
            askForRedirectUrl: async request => {
                const shown = await shortLink(request.authorizationUrl);
                const outcome = await prompt(() => this.prompts.askForRedirectUrl(shown, auth, signal));
                if (outcome.status === 'declined') {
                    throw new AuthorizationDeclinedError(`The user declined to authorize ${auth.name}`);
                }
                if (outcome.status === 'unanswered') {
                    throw signInTimeoutError();
                }
                if (outcome.status === 'pasted') {
                    const invalid = request.validate?.(outcome.url);
                    if (invalid) {
                        throw new Error(invalid);
                    }
                    return outcome.url;
                }
                if (auth.auth_type === 'oauth2_implicit') {
                    throw new McpAuthorizationError(surface.run_request.implicit_needs_dialog.replace('{auth}', auth.name));
                }
                const pasted = this.awaitRedirectUrl(pending, request);
                publish({ url: shown, redirectUrlRequired: true });
                return pasted;
            },
            showError: message => this.log(message),
            log: message => this.log(message)
        };
        pending.token = performOAuth2Flow(auth, new OAuthProvider(this.store, ui)).catch(error => {
            if (pending.waiters === 0 && !signal.aborted) {
                this.failures.set(key, { error, expiresAt: Date.now() + this.redirectUrlTimeoutMs });
            }
            throw error;
        }).finally(() => {
            this.forget(key, pending);
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

function requireSupportedRedirect(auth: OAuthAuthConfig): void {
    const redirectUri = auth.fields.redirect_uri;
    if (isAppRedirect(redirectUri)) {
        throw new McpAuthorizationError(surface.run_request.unsupported_redirect
            .replace('{auth}', auth.name)
            .replace('{redirect_uri}', redirectUri));
    }
}

function delay(ms: number): Promise<void> {
    return new Promise(resolve => setTimeout(resolve, ms).unref());
}

function throwIfCancelled(signal: AbortSignal): void {
    if (signal.aborted) {
        throw new Error(CANCELLED_MESSAGE);
    }
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
