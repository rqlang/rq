import * as vscode from 'vscode';
import * as cliService from '../cliService';

const EP_TEMPLATE_PATTERN = /^\s*ep\s+[a-zA-Z_][a-zA-Z0-9_-]*\s*<\s*([a-zA-Z_][a-zA-Z0-9_-]*)\s*>/;
const EP_DEF_PATTERN = /^\s*ep\s+([a-zA-Z_][a-zA-Z0-9_-]*)/;
const RQ_STATEMENT_PATTERN = /^\s*rq\s+([a-zA-Z_][a-zA-Z0-9_-]*)\s*\(/;
const VAR_NAME_PATTERN = /[a-zA-Z_][a-zA-Z0-9_-]*/;
const VALID_NAME_PATTERN = /^[a-zA-Z_][a-zA-Z0-9_-]*$/;
const KEYWORDS = new Set(['let', 'rq', 'ep', 'env', 'auth', 'import']);

type SymbolKind = 'ep' | 'var' | 'rq';

interface DetectedSymbol {
    kind: SymbolKind;
    name: string;
    range: vscode.Range;
}

function detectSymbol(document: vscode.TextDocument, position: vscode.Position): DetectedSymbol | undefined {
    const line = document.lineAt(position).text;

    const epMatch = line.match(EP_TEMPLATE_PATTERN);
    if (epMatch) {
        const parentName = epMatch[1];
        const angleOpen = line.indexOf('<');
        const parentStart = line.indexOf(parentName, angleOpen);
        const parentEnd = parentStart + parentName.length;
        if (position.character >= parentStart && position.character <= parentEnd) {
            return {
                kind: 'ep',
                name: parentName,
                range: new vscode.Range(
                    new vscode.Position(position.line, parentStart),
                    new vscode.Position(position.line, parentEnd)
                )
            };
        }
    }

    const epDefMatch = line.match(EP_DEF_PATTERN);
    if (epDefMatch) {
        const epName = epDefMatch[1];
        const epKeywordIdx = line.search(/\bep\b/);
        const nameStart = line.indexOf(epName, epKeywordIdx + 2);
        const nameEnd = nameStart + epName.length;
        if (position.character >= nameStart && position.character <= nameEnd) {
            return {
                kind: 'ep',
                name: epName,
                range: new vscode.Range(
                    new vscode.Position(position.line, nameStart),
                    new vscode.Position(position.line, nameEnd)
                )
            };
        }
    }

    const rqMatch = line.match(RQ_STATEMENT_PATTERN);
    if (rqMatch) {
        const rqName = rqMatch[1];
        const rqKeywordIdx = line.search(/\brq\b/);
        const nameStart = line.indexOf(rqName, rqKeywordIdx + 2);
        const nameEnd = nameStart + rqName.length;
        if (position.character >= nameStart && position.character <= nameEnd) {
            return {
                kind: 'rq',
                name: rqName,
                range: new vscode.Range(
                    new vscode.Position(position.line, nameStart),
                    new vscode.Position(position.line, nameEnd)
                )
            };
        }
    }

    const wordRange = document.getWordRangeAtPosition(position, VAR_NAME_PATTERN);
    if (!wordRange) { return undefined; }
    const word = document.getText(wordRange);
    if (KEYWORDS.has(word)) { return undefined; }
    return { kind: 'var', name: word, range: wordRange };
}

export const renameProvider = vscode.languages.registerRenameProvider('rq', {
    prepareRename(document: vscode.TextDocument, position: vscode.Position) {
        const symbol = detectSymbol(document, position);
        if (!symbol) {
            throw new Error('No renameable symbol at this position');
        }
        return { range: symbol.range, placeholder: symbol.name };
    },

    async provideRenameEdits(
        document: vscode.TextDocument,
        position: vscode.Position,
        newName: string
    ): Promise<vscode.WorkspaceEdit | undefined> {
        if (!VALID_NAME_PATTERN.test(newName)) {
            throw new Error(`Invalid name: "${newName}". Name must match [a-zA-Z_][a-zA-Z0-9_-]*`);
        }

        const symbol = detectSymbol(document, position);
        if (!symbol) { return undefined; }

        const sourceDirectory = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
        const edit = new vscode.WorkspaceEdit();

        if (symbol.kind === 'rq') {
            edit.replace(document.uri, symbol.range, newName);
            return edit;
        }

        if (symbol.kind === 'ep') {
            try {
                const [refs, def] = await Promise.all([
                    cliService.epRefs(symbol.name, sourceDirectory),
                    cliService.showEndpoint(symbol.name, sourceDirectory)
                ]);
                edit.replace(
                    vscode.Uri.file(def.file),
                    new vscode.Range(
                        new vscode.Position(def.line, def.character),
                        new vscode.Position(def.line, def.character + symbol.name.length)
                    ),
                    newName
                );
                for (const r of refs) {
                    edit.replace(
                        vscode.Uri.file(r.file),
                        new vscode.Range(
                            new vscode.Position(r.line, r.character),
                            new vscode.Position(r.line, r.character + symbol.name.length)
                        ),
                        newName
                    );
                }
            } catch {
                return undefined;
            }
            return edit;
        }

        try {
            const refs = await cliService.varRefs(symbol.name, sourceDirectory);
            for (const r of refs) {
                edit.replace(
                    vscode.Uri.file(r.file),
                    new vscode.Range(
                        new vscode.Position(r.line, r.character),
                        new vscode.Position(r.line, r.character + symbol.name.length)
                    ),
                    newName
                );
            }
        } catch {
            return undefined;
        }
        return edit;
    }
});
