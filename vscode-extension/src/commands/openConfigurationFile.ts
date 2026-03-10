import * as vscode from 'vscode';
import { normalizePath } from '../utils';

export function registerOpenConfigurationFileCommand(context: vscode.ExtensionContext) {
    const command = vscode.commands.registerCommand('rq.openConfigurationFile', async (filePath: string, blockType: string, name: string) => {
        try {
            const normalizedPath = normalizePath(filePath);
            const document = await vscode.workspace.openTextDocument(normalizedPath);
            const editor = await vscode.window.showTextDocument(document);

            const text = document.getText();
            const pattern = new RegExp(`\\b${blockType}\\s+${name}\\b`);
            const match = pattern.exec(text);

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
