import * as vscode from 'vscode';
import * as rqClient from '../rqClient';
import { RequestExplorerProvider } from '../requestExplorer';
import { pickEnvironment } from '../vscodeUtils';

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

            const choice = await pickEnvironment(
                environments,
                provider.getSelectedEnvironment(),
                'RQ Environment Selection'
            );

            if (choice) {
                provider.setSelectedEnvironment(choice.environment);

                if (choice.environment) {
                    vscode.window.showInformationMessage(`Environment set to: ${choice.environment}`);
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
