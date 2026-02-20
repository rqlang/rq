import * as vscode from 'vscode';
import { OAuthProvider } from '../auth/oauthProvider';

export function registerClearOAuthCacheCommand(context: vscode.ExtensionContext) {
    const command = vscode.commands.registerCommand('rq.clearOAuthCache', async () => {
        try {
            const oauthProvider = new OAuthProvider(context);
            await oauthProvider.clearTokenCache();
            vscode.window.showInformationMessage('RQ: OAuth cache cleared successfully.');
        } catch (error) {
            vscode.window.showErrorMessage(`RQ: Failed to clear OAuth cache: ${error instanceof Error ? error.message : String(error)}`);
        }
    });

    context.subscriptions.push(command);
}
