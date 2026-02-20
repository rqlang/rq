import * as vscode from 'vscode';

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

export function waitForUri(predicate: (uri: vscode.Uri) => boolean, timeoutMs: number = 300000): Promise<vscode.Uri> {
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
            reject(new Error('Authentication timeout - no response received from callback'));
        }, timeoutMs);
    });
}
