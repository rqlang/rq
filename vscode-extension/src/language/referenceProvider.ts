import * as vscode from 'vscode';
import * as cliService from '../cliService';

const EP_TEMPLATE_PATTERN = /^\s*ep\s+[a-zA-Z_][a-zA-Z0-9_-]*\s*<\s*([a-zA-Z_][a-zA-Z0-9_-]*)\s*>/;
const VAR_NAME_PATTERN = /[a-zA-Z_][a-zA-Z0-9_]*/;

export const referenceProvider = vscode.languages.registerReferenceProvider('rq', {
    async provideReferences(document: vscode.TextDocument, position: vscode.Position) {
        const line = document.lineAt(position).text;
        const sourceDirectory = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;

        const epMatch = line.match(EP_TEMPLATE_PATTERN);
        if (epMatch) {
            const parentName = epMatch[1];
            const angleOpen = line.indexOf('<');
            const parentStart = line.indexOf(parentName, angleOpen);
            const parentEnd = parentStart + parentName.length;

            if (position.character >= parentStart && position.character <= parentEnd) {
                try {
                    const refs = await cliService.epRefs(parentName, sourceDirectory);
                    return refs.map(r => new vscode.Location(
                        vscode.Uri.file(r.file),
                        new vscode.Position(r.line, r.character)
                    ));
                } catch {
                    return [];
                }
            }
        }

        const wordRange = document.getWordRangeAtPosition(position, VAR_NAME_PATTERN);
        if (!wordRange) {
            return [];
        }
        const word = document.getText(wordRange);

        try {
            const refs = await cliService.varRefs(word, sourceDirectory);
            return refs.map(r => new vscode.Location(
                vscode.Uri.file(r.file),
                new vscode.Position(r.line, r.character)
            ));
        } catch {
            return [];
        }
    }
});
