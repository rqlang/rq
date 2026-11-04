import { authUriHandler, waitForUri } from '../../src/auth/authUriHandler';
import { CANCELLED_MESSAGE } from '../../src/utils';
import * as vscode from 'vscode';

describe('waitForUri', () => {
    test('resolves with the first uri the predicate accepts', async () => {
        const waiting = waitForUri(uri => uri.query.includes('code'));
        authUriHandler.handleUri(vscode.Uri.parse('vscode://rq-lang.rq-language/oauth-callback?other=1'));
        authUriHandler.handleUri(vscode.Uri.parse('vscode://rq-lang.rq-language/oauth-callback?code=foo'));
        expect((await waiting).query).toBe('code=foo');
    });

    test('stops waiting once cancelled', async () => {
        const cancellation = new AbortController();
        const waiting = waitForUri(() => true, 60_000, cancellation.signal);
        cancellation.abort();
        await expect(waiting).rejects.toThrow(CANCELLED_MESSAGE);
    });

    test('does not wait when cancelled before it started', async () => {
        const cancellation = new AbortController();
        cancellation.abort();
        await expect(waitForUri(() => true, 60_000, cancellation.signal)).rejects.toThrow(CANCELLED_MESSAGE);
    });
});
