import * as vscode from 'vscode';
import { normalizePath } from '../utils';
import { RequestExplorerProvider, RequestTreeItem } from '../requestExplorer';

export function registerOpenEndpointCommand(context: vscode.ExtensionContext, provider: RequestExplorerProvider) {
    const command = vscode.commands.registerCommand('rq.openEndpoint', async (file: string, line: number, character: number, item?: RequestTreeItem) => {
        if (item) { provider.setItemLoading(item, true); }
        try {
            const document = await vscode.workspace.openTextDocument(normalizePath(file));
            const editor = await vscode.window.showTextDocument(document);
            const position = new vscode.Position(line, character);
            editor.selection = new vscode.Selection(position, position);
            editor.revealRange(new vscode.Range(position, position), vscode.TextEditorRevealType.InCenter);
        } catch (error) {
            vscode.window.showErrorMessage(`Failed to open file: ${error instanceof Error ? error.message : 'Unknown error'}`);
        } finally {
            if (item) { provider.setItemLoading(item, false); }
        }
    });
    context.subscriptions.push(command);
}
