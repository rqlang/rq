import * as vscode from 'vscode';
import * as cliService from '../cliService';

const EP_TEMPLATE_PATTERN = /^\s*ep\s+[a-zA-Z_][a-zA-Z0-9_-]*\s*<\s*([a-zA-Z_][a-zA-Z0-9_-]*)\s*>/;

export const definitionProvider = vscode.languages.registerDefinitionProvider('rq', {
    async provideDefinition(document: vscode.TextDocument, position: vscode.Position) {
        const line = document.lineAt(position).text;
        const epMatch = line.match(EP_TEMPLATE_PATTERN);
        if (!epMatch) {
            return null;
        }

        const parentName = epMatch[1];
        const angleOpen = line.indexOf('<');
        const parentStart = line.indexOf(parentName, angleOpen);
        const parentEnd = parentStart + parentName.length;

        if (position.character < parentStart || position.character > parentEnd) {
            return null;
        }

        try {
            const sourceDirectory = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
            const result = await cliService.showEndpoint(parentName, sourceDirectory);
            return new vscode.Location(
                vscode.Uri.file(result.file),
                new vscode.Position(result.line, result.character)
            );
        } catch {
            return null;
        }
    }
});
