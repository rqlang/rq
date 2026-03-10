import * as vscode from 'vscode';
import * as cliService from './cliService';
import { RequestExplorerProvider, RequestTreeItem } from './requestExplorer';
import { ConfigurationExplorerProvider } from './configurationExplorer';
import { completionProvider } from './language/completionProvider';
import { hoverProvider } from './language/hoverProvider';
import { registerRefreshRequestsCommand } from './commands/refreshRequests';
import { registerOpenRequestFileCommand } from './commands/openRequestFile';
import { registerOpenConfigurationFileCommand } from './commands/openConfigurationFile';
import { registerSelectEnvironmentCommand } from './commands/selectEnvironment';
import { RequestRunner } from './commands/runRequest';
import { registerGetTokenCommand } from './commands/getToken';
import { registerClearOAuthCacheCommand } from './commands/clearOAuthCache';
import { registerAuthUriHandler } from './auth/authUriHandler';
import { normalizePath } from './utils';

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
    context.subscriptions.push(
        vscode.commands.registerCommand('rq.openEndpoint', async (file: string, line: number, character: number, item?: RequestTreeItem) => {
            if (item) { requestExplorerProvider.setItemLoading(item, true); }
            try {
                const document = await vscode.workspace.openTextDocument(normalizePath(file));
                const editor = await vscode.window.showTextDocument(document);
                const position = new vscode.Position(line, character);
                editor.selection = new vscode.Selection(position, position);
                editor.revealRange(new vscode.Range(position, position), vscode.TextEditorRevealType.InCenter);
            } catch (error) {
                vscode.window.showErrorMessage(`Failed to open file: ${error instanceof Error ? error.message : 'Unknown error'}`);
            } finally {
                if (item) { requestExplorerProvider.setItemLoading(item, false); }
            }
        })
    );
    context.subscriptions.push(
        vscode.commands.registerCommand('rq.refreshConfiguration', () => configurationExplorerProvider.refresh())
    );
    registerSelectEnvironmentCommand(context, requestExplorerProvider);
    
    const requestRunner = new RequestRunner(context, rqOutputChannel);
    requestRunner.registerCommands(requestExplorerProvider);

    registerGetTokenCommand(context, rqOutputChannel);
    registerClearOAuthCacheCommand(context);

    // Note: OAuth sessions are not cached in VS Code's authentication API
    // Each "Get Token" command executes a fresh OAuth flow using CLI configuration

    context.subscriptions.push(completionProvider, hoverProvider);
}




