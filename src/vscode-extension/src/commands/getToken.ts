import * as vscode from 'vscode';
import * as rqClient from '../rqClient';
import { performOAuth2Flow } from '../auth';

export function registerGetTokenCommand(context: vscode.ExtensionContext, outputChannel: vscode.OutputChannel) {
    const command = vscode.commands.registerCommand('rq.getToken', async () => {
        try {
            console.log('Getting auth token from CLI...');
            
            // Get the current workspace folder
            const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
            const sourceDirectory = workspaceFolder?.uri.fsPath;
            
            // Step 1: Get environments
            console.log('Fetching environments...');
            const environments = await rqClient.listEnvironments(sourceDirectory);
            
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
            const authConfigs = await rqClient.listAuthConfigs(sourceDirectory);
            
            if (authConfigs.length === 0) {
                vscode.window.showWarningMessage('No auth configurations found in the workspace.');
                return;
            }
            
            // Step 3: Filter for supported OAuth2 configurations
            const oauthConfigs = authConfigs
                .filter(config => 
                    config.auth_type === 'oauth2_authorization_code' || 
                    config.auth_type === 'oauth2_implicit'
                )
                .map(config => config.name);

            if (oauthConfigs.length === 0) {
                vscode.window.showWarningMessage('No supported OAuth2 configurations found (authorization_code or implicit).');
                return;
            }

            const selectedAuthName = await vscode.window.showQuickPick(oauthConfigs, {
                placeHolder: 'Select an OAuth2 authentication configuration',
                title: 'Get OAuth2 Token'
            });
            
            if (!selectedAuthName) {
                // User cancelled
                return;
            }
            
            // Step 4: Get auth details
            console.log(`Fetching auth config: ${selectedAuthName}`);
            const authConfig = await rqClient.showAuthConfig(
                selectedAuthName,
                sourceDirectory,
                selectedEnvironment
            );
            
            // Step 5: Perform OAuth2 flow
            console.log(`Starting OAuth2 flow for auth type: ${authConfig.auth_type}`);

            let accessToken: string;
            try {
                accessToken = await vscode.window.withProgress({
                    location: vscode.ProgressLocation.Notification,
                    title: 'Getting OAuth2 access token...',
                    cancellable: false
                }, () => performOAuth2Flow(authConfig, context, outputChannel));
            } catch (error) {
                const errorMessage = error instanceof Error ? error.message : 'Unknown error';
                console.error('OAuth2 flow failed:', errorMessage);
                vscode.window.showErrorMessage(`OAuth2 authentication failed: ${errorMessage}`);
                return;
            }

            const action = await vscode.window.showInformationMessage(
                `OAuth2 authentication successful! Access token obtained.`,
                'Copy Token'
            );

            if (action === 'Copy Token') {
                await vscode.env.clipboard.writeText(accessToken);
                vscode.window.showInformationMessage('Access token copied to clipboard');
            }
            
        } catch (error) {
            const errorMessage = error instanceof Error ? error.message : 'Unknown error';
            console.error('Failed to get token:', errorMessage);
            vscode.window.showErrorMessage(`Failed to get auth token: ${errorMessage}`);
        }
    });
    context.subscriptions.push(command);
}
