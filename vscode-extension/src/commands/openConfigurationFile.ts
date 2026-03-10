import * as vscode from 'vscode';
import * as cliService from '../cliService';
import { normalizePath } from '../utils';

export function registerOpenConfigurationFileCommand(context: vscode.ExtensionContext) {
    const command = vscode.commands.registerCommand('rq.openConfigurationFile', async (artifactType: 'env' | 'auth', name: string) => {
        try {
            const sourceDirectory = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
            let file: string;
            let line: number;
            let character: number;

            if (artifactType === 'env') {
                const result = await cliService.showEnvironment(name, sourceDirectory);
                file = result.file;
                line = result.line;
                character = result.character;
            } else {
                const result = await cliService.showAuthConfig(name, sourceDirectory);
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
            vscode.window.showErrorMessage(`Failed to open file: ${error instanceof Error ? error.message : 'Unknown error'}`);
        }
    });
    context.subscriptions.push(command);
}
