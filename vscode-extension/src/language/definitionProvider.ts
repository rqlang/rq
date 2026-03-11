import * as vscode from 'vscode';
import * as cliService from '../cliService';
import { parseVariables } from './definitions';

const EP_TEMPLATE_PATTERN = /^\s*ep\s+[a-zA-Z_][a-zA-Z0-9_-]*\s*<\s*([a-zA-Z_][a-zA-Z0-9_-]*)\s*>/;
const VAR_NAME_PATTERN = /[a-zA-Z_][a-zA-Z0-9_]*/;

let environmentProvider: { getSelectedEnvironment(): string | undefined } | undefined;

export function setEnvironmentProvider(provider: { getSelectedEnvironment(): string | undefined }) {
    environmentProvider = provider;
}

export const definitionProvider = vscode.languages.registerDefinitionProvider('rq', {
    async provideDefinition(document: vscode.TextDocument, position: vscode.Position) {
        const line = document.lineAt(position).text;

        const epMatch = line.match(EP_TEMPLATE_PATTERN);
        if (epMatch) {
            const parentName = epMatch[1];
            const angleOpen = line.indexOf('<');
            const parentStart = line.indexOf(parentName, angleOpen);
            const parentEnd = parentStart + parentName.length;

            if (position.character >= parentStart && position.character <= parentEnd) {
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
        }

        const wordRange = document.getWordRangeAtPosition(position, VAR_NAME_PATTERN);
        if (!wordRange) {
            return null;
        }
        const word = document.getText(wordRange);

        const sourceDirectory = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
        const environment = environmentProvider?.getSelectedEnvironment();

        if (!environment) {
            const localVars = parseVariables(document);
            const localVar = localVars.find(v => v.name === word);
            if (localVar) {
                return new vscode.Location(
                    document.uri,
                    new vscode.Position(localVar.line, 0)
                );
            }
        }

        try {
            const result = await cliService.showVariable(word, sourceDirectory, environment, false);
            return new vscode.Location(
                vscode.Uri.file(result.file),
                new vscode.Position(result.line, result.character)
            );
        } catch {
            return null;
        }
    }
});
