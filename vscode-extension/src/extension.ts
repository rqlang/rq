import * as vscode from 'vscode';
import * as cliService from './cliService';
import { RequestExplorerProvider } from './requestExplorer';
import { completionProvider } from './language/completionProvider';
import { hoverProvider } from './language/hoverProvider';
import { registerRefreshRequestsCommand } from './commands/refreshRequests';
import { registerOpenRequestFileCommand } from './commands/openRequestFile';
import { registerSelectEnvironmentCommand } from './commands/selectEnvironment';
import { RequestRunner } from './commands/runRequest';
import { registerGetTokenCommand } from './commands/getToken';
import { registerClearOAuthCacheCommand } from './commands/clearOAuthCache';
import { registerAuthUriHandler } from './auth/authUriHandler';

export function activate(context: vscode.ExtensionContext) {
    console.log('RQ Language Extension is now active');

    const rqOutputChannel = vscode.window.createOutputChannel('RQ');
    context.subscriptions.push(rqOutputChannel);

    registerAuthUriHandler(context);

    cliService.setExtensionMode(context.extensionMode);
    cliService.setExtensionPath(context.extensionPath);

    cliService.setOutputChannel(rqOutputChannel);

    const diagnosticCollection = vscode.languages.createDiagnosticCollection('rq');
    context.subscriptions.push(diagnosticCollection);
    cliService.setDiagnosticCollection(diagnosticCollection);

    const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    const requestExplorerProvider = new RequestExplorerProvider(workspaceRoot);
    const requestExplorerView = vscode.window.createTreeView('rqRequestExplorer', {
        treeDataProvider: requestExplorerProvider,
        showCollapseAll: true
    });
    context.subscriptions.push(requestExplorerView);

    cliService.checkCliVersion('rq-lang.rq-language').then(() => {
        requestExplorerProvider.refresh();
    });

    cliService.onInstallFinished(() => {
        requestExplorerProvider.refresh();
    });

    // Register commands
    registerRefreshRequestsCommand(context, requestExplorerProvider);
    registerOpenRequestFileCommand(context);
    registerSelectEnvironmentCommand(context, requestExplorerProvider);
    
    const requestRunner = new RequestRunner(context, rqOutputChannel);
    requestRunner.registerCommands(requestExplorerProvider);

    registerGetTokenCommand(context, rqOutputChannel);
    registerClearOAuthCacheCommand(context);

    // Note: OAuth sessions are not cached in VS Code's authentication API
    // Each "Get Token" command executes a fresh OAuth flow using CLI configuration

    context.subscriptions.push(completionProvider, hoverProvider);
}




