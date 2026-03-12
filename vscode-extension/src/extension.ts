import * as vscode from 'vscode';
import * as cliService from './cliService';
import { RequestExplorerProvider } from './requestExplorer';
import { ConfigurationExplorerProvider } from './configurationExplorer';
import { completionProvider } from './language/completionProvider';
import { hoverProvider, setEnvironmentProvider as setHoverEnvironmentProvider } from './language/hoverProvider';
import { definitionProvider, setEnvironmentProvider } from './language/definitionProvider';
import { referenceProvider } from './language/referenceProvider';
import { registerRefreshRequestsCommand } from './commands/refreshRequests';
import { registerOpenRequestFileCommand } from './commands/openRequestFile';
import { registerOpenConfigurationFileCommand } from './commands/openConfigurationFile';
import { registerOpenEndpointCommand } from './commands/openEndpoint';
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

    const configurationExplorerProvider = new ConfigurationExplorerProvider(workspaceRoot);
    const configurationExplorerView = vscode.window.createTreeView('rqConfigurationExplorer', {
        treeDataProvider: configurationExplorerProvider,
        showCollapseAll: true
    });
    context.subscriptions.push(configurationExplorerView);

    cliService.checkCliVersion('rq-lang.rq-language').then(() => {
        requestExplorerProvider.refresh();
        configurationExplorerProvider.refresh();
    });

    cliService.onInstallFinished(() => {
        requestExplorerProvider.refresh();
        configurationExplorerProvider.refresh();
    });

    // Register commands
    registerRefreshRequestsCommand(context, requestExplorerProvider);
    registerOpenRequestFileCommand(context, requestExplorerProvider);
    registerOpenConfigurationFileCommand(context, configurationExplorerProvider);
    registerOpenEndpointCommand(context, requestExplorerProvider);
    context.subscriptions.push(
        vscode.commands.registerCommand('rq.refreshConfiguration', () => configurationExplorerProvider.refresh())
    );
    registerSelectEnvironmentCommand(context, requestExplorerProvider);
    
    const requestRunner = new RequestRunner(context, rqOutputChannel);
    requestRunner.registerCommands(requestExplorerProvider);

    setEnvironmentProvider(requestExplorerProvider);
    setHoverEnvironmentProvider(requestExplorerProvider);

    registerGetTokenCommand(context, rqOutputChannel);
    registerClearOAuthCacheCommand(context);

    // Note: OAuth sessions are not cached in VS Code's authentication API
    // Each "Get Token" command executes a fresh OAuth flow using CLI configuration

    context.subscriptions.push(completionProvider, hoverProvider, definitionProvider, referenceProvider);
}




