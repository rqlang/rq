import * as vscode from 'vscode';
import * as cliService from '../cliService';
import { performOAuth2Flow } from '../auth';

export function registerGetTokenCommand(context: vscode.ExtensionContext, outputChannel: vscode.OutputChannel) {
    const command = vscode.commands.registerCommand('rq.getToken', async () => {
        if (cliService.isCliInstalling()) {
            vscode.window.showWarningMessage('rq CLI is being installed. Please wait until installation completes.');
            return;
        }

        if (!cliService.isCliBinaryAvailable()) {
            await cliService.handleCliNotFoundError();
            return;
        }

        try {
            console.log('Getting auth token from CLI...');
            
            // Get the current workspace folder
            const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
            const sourceDirectory = workspaceFolder?.uri.fsPath;
            
            // Step 1: Get environments
            console.log('Fetching environments...');
            const environments = await cliService.listEnvironments(sourceDirectory);
            
            let selectedEnvironment: string | undefined;
            
            if (environments.length > 0) {
                // Show environment selection (optional)
                const envChoice = await vscode.window.showQuickPick(
                    ['(No environment)', ...environments],
                    {
                        placeHolder: 'Select an environment (optional)',
                        title: 'Choose Environment'
                    }
                );
                
                if (envChoice === undefined) {
                    // User cancelled
                    return;
                }
                
                selectedEnvironment = envChoice === '(No environment)' ? undefined : envChoice;
            }
            
            // Step 2: Get auth configurations
            console.log('Fetching auth configurations...');
            const authConfigs = await cliService.listAuthConfigs(sourceDirectory);
            
            if (authConfigs.length === 0) {
                vscode.window.showWarningMessage('No auth configurations found in the workspace.');
                return;
            }
            
            // Step 3: Let user choose an auth config (required)
            const selectedAuthName = await vscode.window.showQuickPick(authConfigs, {
                placeHolder: 'Select an authentication configuration',
                title: 'Choose Authentication'
            });
            
            if (!selectedAuthName) {
                // User cancelled
                return;
            }
            
            // Step 4: Get auth details
            console.log(`Fetching auth config: ${selectedAuthName}`);
            const authConfig = await cliService.showAuthConfig(
                selectedAuthName,
                sourceDirectory,
                selectedEnvironment
            );
            
            // Step 5: Perform OAuth2 flow (flow type determined by auth_type)
            console.log(`Starting OAuth2 flow for auth type: ${authConfig.auth_type}`);
            vscode.window.showInformationMessage('Starting OAuth2 authentication flow...');
            
            try {
                const accessToken = await performOAuth2Flow(authConfig, context, outputChannel);
                
                // Step 7: Show success and offer to copy token
                const action = await vscode.window.showInformationMessage(
                    'OAuth2 authentication successful! Access token obtained.',
                    'Copy Token'
                );
                
                if (action === 'Copy Token') {
                    await vscode.env.clipboard.writeText(accessToken);
                    vscode.window.showInformationMessage('Access token copied to clipboard');
                }
            } catch (error) {
                const errorMessage = error instanceof Error ? error.message : 'Unknown error';
                console.error('OAuth2 flow failed:', errorMessage);
                vscode.window.showErrorMessage(`OAuth2 authentication failed: ${errorMessage}`);
            }
            
        } catch (error) {
            const errorMessage = error instanceof Error ? error.message : 'Unknown error';
            console.error('Failed to get token:', errorMessage);
            vscode.window.showErrorMessage(`Failed to get auth token: ${errorMessage}`);
        }
    });
    context.subscriptions.push(command);
}
