import * as vscode from 'vscode';
import * as cliService from '../cliService';
import { normalizePath } from '../utils';

export function registerOpenRequestFileCommand(context: vscode.ExtensionContext) {
    const command = vscode.commands.registerCommand('rq.openRequestFile', async (requestName: string) => {
        try {
            const sourceDirectory = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
            const result = await cliService.showRequest(requestName, sourceDirectory);
            const normalizedPath = normalizePath(result.file);
            const document = await vscode.workspace.openTextDocument(normalizedPath);
            const editor = await vscode.window.showTextDocument(document);
            const position = new vscode.Position(result.line, result.character);
            editor.selection = new vscode.Selection(position, position);
            editor.revealRange(new vscode.Range(position, position), vscode.TextEditorRevealType.InCenter);
        } catch (error) {
            vscode.window.showErrorMessage(`Failed to open file: ${error instanceof Error ? error.message : 'Unknown error'}`);
        }
    });
    context.subscriptions.push(command);
}
