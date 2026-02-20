import * as vscode from 'vscode';
import { normalizePath } from '../utils';

export function registerOpenRequestFileCommand(context: vscode.ExtensionContext) {
    const command = vscode.commands.registerCommand('rq.openRequestFile', async (filePath: string, requestName: string) => {
        try {
            const normalizedPath = normalizePath(filePath);
            const document = await vscode.workspace.openTextDocument(normalizedPath);
            const editor = await vscode.window.showTextDocument(document);
            
            // Try to find and highlight the request in the file
            const text = document.getText();
            const requestPattern = new RegExp(`\\brq\\s+${requestName}\\b`);
            const match = requestPattern.exec(text);
            
            if (match) {
                const position = document.positionAt(match.index);
                editor.selection = new vscode.Selection(position, position);
                editor.revealRange(new vscode.Range(position, position), vscode.TextEditorRevealType.InCenter);
            }
        } catch (error) {
            vscode.window.showErrorMessage(`Failed to open file: ${error instanceof Error ? error.message : 'Unknown error'}`);
        }
    });
    context.subscriptions.push(command);
}
