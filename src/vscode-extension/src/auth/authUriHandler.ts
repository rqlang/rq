import * as vscode from 'vscode';
import { SIGN_IN_TIMEOUT_MS, signInTimeoutError } from './signInErrors';

export interface UriHandlerCallback {
    (uri: vscode.Uri): void;
}

class AuthUriHandler implements vscode.UriHandler {
    private _pendingCallbacks: Map<string, UriHandlerCallback> = new Map();
    private _onDidHandleUri = new vscode.EventEmitter<vscode.Uri>();
    public readonly onDidHandleUri = this._onDidHandleUri.event;

    handleUri(uri: vscode.Uri): void {
        console.log('AuthURIHandler received URI:', uri.toString());
        this._onDidHandleUri.fire(uri);
    }
}

export const authUriHandler = new AuthUriHandler();

export function registerAuthUriHandler(context: vscode.ExtensionContext) {
    context.subscriptions.push(
        vscode.window.registerUriHandler(authUriHandler)
    );
}

export function waitForUri(predicate: (uri: vscode.Uri) => boolean, timeoutMs: number = SIGN_IN_TIMEOUT_MS): Promise<vscode.Uri> {
    return new Promise((resolve, reject) => {
        const disposable = authUriHandler.onDidHandleUri(uri => {
            if (predicate(uri)) {
                disposable.dispose();
                clearTimeout(timeoutHandle);
                resolve(uri);
            }
        });

        const timeoutHandle = setTimeout(() => {
            disposable.dispose();
            reject(signInTimeoutError());
        }, timeoutMs);
    });
}
