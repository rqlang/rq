import * as vscode from 'vscode';
import * as rqClient from '../rqClient';
import { RequestExplorerProvider } from '../requestExplorer';

export function registerSelectEnvironmentCommand(context: vscode.ExtensionContext, provider: RequestExplorerProvider) {
    const command = vscode.commands.registerCommand('rq.selectEnvironment', async () => {
        try {
            const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
            const sourceDirectory = workspaceFolder?.uri.fsPath;

            provider.setTreeLoading(true);
            let environments: string[];
            try {
                environments = await rqClient.listEnvironments(sourceDirectory);
            } finally {
                provider.setTreeLoading(false);
            }

            // Create quick pick items with "None" as the first option
            const items: vscode.QuickPickItem[] = [
                { label: 'None', description: 'No environment selected' }
            ];

            // Add environments from CLI
            for (const env of environments) {
                items.push({ label: env });
            }

            // Show quick pick
            const selected = await vscode.window.showQuickPick(items, {
                placeHolder: 'Select an environment',
                title: 'RQ Environment Selection'
            });

            if (selected) {
                // Set environment in provider (undefined if "None" selected)
                const environment = selected.label === 'None' ? undefined : selected.label;
                provider.setSelectedEnvironment(environment);

                // Show confirmation
                if (environment) {
                    vscode.window.showInformationMessage(`Environment set to: ${environment}`);
                } else {
                    vscode.window.showInformationMessage('No environment selected');
                }
            }
        } catch (error) {
            provider.setTreeLoading(false);
            vscode.window.showErrorMessage(`Failed to select environment: ${error instanceof Error ? error.message : 'Unknown error'}`);
        }
    });
    context.subscriptions.push(command);
}
