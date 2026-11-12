import * as vscode from 'vscode';
import { normalizePath } from '../utils';
import { RequestExplorerProvider, RequestTreeItem } from '../requestExplorer';

export function registerOpenRequestFileCommand(context: vscode.ExtensionContext, provider: RequestExplorerProvider) {
    const command = vscode.commands.registerCommand('rq.openRequestFile', async (item: RequestTreeItem) => {
        if (!item?.request) {
            vscode.window.showErrorMessage('Cannot open: No request information available');
            return;
        }
        provider.setItemLoading(item, true);
        try {
            const document = await vscode.workspace.openTextDocument(normalizePath(item.request.request_file));
            const editor = await vscode.window.showTextDocument(document);
            const position = new vscode.Position(item.request.line, item.request.character);
            editor.selection = new vscode.Selection(position, position);
            editor.revealRange(new vscode.Range(position, position), vscode.TextEditorRevealType.InCenter);
        } catch (error) {
            vscode.window.showErrorMessage(error instanceof Error ? error.message : 'Unknown error');
        } finally {
            provider.setItemLoading(item, false);
        }
    });
    context.subscriptions.push(command);
}
