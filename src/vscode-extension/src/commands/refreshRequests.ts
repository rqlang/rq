import * as vscode from 'vscode';
import { RequestExplorerProvider } from '../requestExplorer';

export function registerRefreshRequestsCommand(context: vscode.ExtensionContext, provider: RequestExplorerProvider) {
    const command = vscode.commands.registerCommand('rq.refreshRequests', async () => {
        provider.refresh();
    });
    context.subscriptions.push(command);
}
