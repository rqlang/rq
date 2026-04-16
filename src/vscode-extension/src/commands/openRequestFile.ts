import * as vscode from 'vscode';
import * as rqClient from '../rqClient';
import { normalizePath } from '../utils';
import { RequestExplorerProvider, RequestTreeItem } from '../requestExplorer';

export function registerOpenRequestFileCommand(context: vscode.ExtensionContext, provider: RequestExplorerProvider) {
    const command = vscode.commands.registerCommand('rq.openRequestFile', async (requestName: string, item?: RequestTreeItem) => {
        if (item) { provider.setItemLoading(item, true); }
        try {
            const sourceDirectory = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
            const result = await rqClient.showRequestLocation(requestName, sourceDirectory);
            const normalizedPath = normalizePath(result.file);
            const document = await vscode.workspace.openTextDocument(normalizedPath);
            const editor = await vscode.window.showTextDocument(document);
            const position = new vscode.Position(result.line, result.character);
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
