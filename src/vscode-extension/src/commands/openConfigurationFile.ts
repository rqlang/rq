import * as vscode from 'vscode';
import * as rqClient from '../rqClient';
import { normalizePath } from '../utils';
import { ConfigurationExplorerProvider, ConfigurationTreeItem } from '../configurationExplorer';

export function registerOpenConfigurationFileCommand(context: vscode.ExtensionContext, provider: ConfigurationExplorerProvider) {
    const command = vscode.commands.registerCommand('rq.openConfigurationFile', async (artifactType: 'env' | 'auth', name: string, item?: ConfigurationTreeItem) => {
        if (item) { provider.setItemLoading(item, true); }
        try {
            const sourceDirectory = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
            let file: string;
            let line: number;
            let character: number;

            if (artifactType === 'env') {
                const result = await rqClient.showEnvironment(name, sourceDirectory);
                file = result.file;
                line = result.line;
                character = result.character;
            } else {
                const result = await rqClient.showAuthLocation(name, sourceDirectory);
                file = result.file;
                line = result.line;
                character = result.character;
            }

            const normalizedPath = normalizePath(file);
            const document = await vscode.workspace.openTextDocument(normalizedPath);
            const editor = await vscode.window.showTextDocument(document);
            const position = new vscode.Position(line, character);
            editor.selection = new vscode.Selection(position, position);
            editor.revealRange(new vscode.Range(position, position), vscode.TextEditorRevealType.InCenter);
        } catch (error) {
            vscode.window.showErrorMessage(error instanceof Error ? error.message : 'Unknown error');
        } finally {
            if (item) { provider.setItemLoading(item, false); }
        }
    });
    context.subscriptions.push(command);
}
