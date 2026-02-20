import * as vscode from 'vscode';
import * as cliService from '../cliService';
import { RequestExplorerProvider } from '../requestExplorer';

export function registerSelectEnvironmentCommand(context: vscode.ExtensionContext, provider: RequestExplorerProvider) {
    const command = vscode.commands.registerCommand('rq.selectEnvironment', async () => {
        if (cliService.isCliInstalling()) {
            vscode.window.showWarningMessage('rq CLI is being installed. Please wait until installation completes.');
            return;
        }

        if (!cliService.isCliBinaryAvailable()) {
            await cliService.handleCliNotFoundError();
            return;
        }

        try {
            const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
            const sourceDirectory = workspaceFolder?.uri.fsPath;
            
            const environments = await cliService.listEnvironments(sourceDirectory);
            
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
            vscode.window.showErrorMessage(`Failed to select environment: ${error instanceof Error ? error.message : 'Unknown error'}`);
        }
    });
    context.subscriptions.push(command);
}
