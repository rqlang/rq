import * as vscode from 'vscode';
import * as cliService from './cliService';
import { RequestExplorerProvider } from './requestExplorer';
import { ConfigurationExplorerProvider } from './configurationExplorer';
import { completionProvider, insideArrayLiteral, setEnvironmentProvider as setCompletionEnvironmentProvider } from './language/completionProvider';
import { hoverProvider, setEnvironmentProvider as setHoverEnvironmentProvider } from './language/hoverProvider';
import { definitionProvider, setEnvironmentProvider } from './language/definitionProvider';
import { referenceProvider } from './language/referenceProvider';
import { renameProvider } from './language/renameProvider';
import { signatureHelpProvider } from './language/signatureHelpProvider';
import { formattingProvider } from './language/formattingProvider';
import { DiagnosticsProvider } from './language/diagnosticsProvider';
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

    const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    const requestExplorerProvider = new RequestExplorerProvider(workspaceRoot);

    const diagnosticsProvider = new DiagnosticsProvider(diagnosticCollection, requestExplorerProvider);
    context.subscriptions.push({ dispose: () => diagnosticsProvider.dispose() });
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
        diagnosticsProvider.validateAllFolders();
    });

    cliService.onInstallFinished(() => {
        requestExplorerProvider.refresh();
        configurationExplorerProvider.refresh();
        diagnosticsProvider.validateAllFolders();
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
    context.subscriptions.push(
        requestExplorerProvider.onDidChangeEnvironment(() => diagnosticsProvider.validateAllFolders())
    );
    
    const requestRunner = new RequestRunner(context, rqOutputChannel);
    requestRunner.registerCommands(requestExplorerProvider);

    setEnvironmentProvider(requestExplorerProvider);
    setHoverEnvironmentProvider(requestExplorerProvider);
    setCompletionEnvironmentProvider(requestExplorerProvider);

    registerGetTokenCommand(context, rqOutputChannel);
    registerClearOAuthCacheCommand(context);

    // Note: OAuth sessions are not cached in VS Code's authentication API
    // Each "Get Token" command executes a fresh OAuth flow using CLI configuration

    const validationOnChange = vscode.workspace.onDidChangeTextDocument(event => {
        diagnosticsProvider.scheduleValidation(event.document);
    });

    const validationOnSave = vscode.workspace.onDidSaveTextDocument(document => {
        diagnosticsProvider.validateSaved(document);
    });

    const validationOnOpen = vscode.workspace.onDidOpenTextDocument(document => {
        diagnosticsProvider.validateSaved(document);
    });

    const headerArrayNewlineTrigger = vscode.workspace.onDidChangeTextDocument(event => {
        if (event.document.languageId !== 'rq') { return; }
        const editor = vscode.window.activeTextEditor;
        if (!editor || editor.document !== event.document) { return; }

        const newlineChange = event.contentChanges.find(c => c.text.includes('\n'));
        if (!newlineChange) { return; }

        const insertedLines = newlineChange.text.split('\n');
        const endsWithNewline = newlineChange.text.endsWith('\n');
        const cursorLineOffset = (endsWithNewline && insertedLines.length > 2) ? insertedLines.length - 2 : insertedLines.length - 1;
        const cursorLine = newlineChange.range.start.line + cursorLineOffset;
        if (cursorLine === 0) { return; }

        const cursorLineText = event.document.lineAt(cursorLine).text;
        if (cursorLineText.trim() !== '') { return; }

        const cursorPosition = new vscode.Position(cursorLine, cursorLineText.length);
        const blockText = event.document.getText(new vscode.Range(
            new vscode.Position(Math.max(0, cursorLine - 30), 0),
            cursorPosition
        ));
        if (insideArrayLiteral(blockText)) {
            setTimeout(() => vscode.commands.executeCommand('editor.action.triggerSuggest'), 50);
        }
    });

    context.subscriptions.push(completionProvider, hoverProvider, definitionProvider, referenceProvider, renameProvider, signatureHelpProvider, formattingProvider, headerArrayNewlineTrigger, validationOnChange, validationOnSave, validationOnOpen);
}




