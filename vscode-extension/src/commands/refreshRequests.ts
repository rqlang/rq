import * as vscode from 'vscode';
import * as cliService from '../cliService';
import { RequestExplorerProvider } from '../requestExplorer';

export function registerRefreshRequestsCommand(context: vscode.ExtensionContext, provider: RequestExplorerProvider) {
    const command = vscode.commands.registerCommand('rq.refreshRequests', async () => {
        if (cliService.isCliInstalling()) {
            vscode.window.showWarningMessage('rq CLI is being installed. Please wait until installation completes.');
            return;
        }

        if (!cliService.isCliBinaryAvailable()) {
            await cliService.handleCliNotFoundError();
            provider.refresh();
            return;
        }

        provider.refresh();
    });
    context.subscriptions.push(command);
}
