import * as vscode from 'vscode';
import * as path from 'path';
import * as rqClient from '../rqClient';
import { DraftFile, isInsideFolder } from '../utils';
import { filterRequiredVars } from './completionHelpers';

export interface CompletionContext {
    document: vscode.TextDocument;
    position: vscode.Position;
    linePrefix: string;
    documentPrefix: string;
    triggerKind: vscode.CompletionTriggerKind;
    filePath: string;
    rootPath: string;
    drafts: DraftFile[];
    getEnvironment(): string | undefined;
}

export type EnvironmentProvider = { getSelectedEnvironment(): string | undefined };

function unsavedDrafts(workspaceRoot: string): DraftFile[] {
    return vscode.workspace.textDocuments
        .filter(doc => doc.languageId === 'rq' && doc.isDirty && isInsideFolder(doc.uri.fsPath, workspaceRoot))
        .map(doc => ({ path: doc.uri.fsPath, source: doc.getText() }));
}

export function buildContext(
    document: vscode.TextDocument,
    position: vscode.Position,
    vsContext: vscode.CompletionContext | undefined,
    environmentProvider: EnvironmentProvider | undefined
): CompletionContext {
    const rootPath = vscode.workspace.getWorkspaceFolder(document.uri)?.uri.fsPath
        ?? path.dirname(document.uri.fsPath);
    let drafts: DraftFile[] | undefined;
    return {
        document,
        position,
        linePrefix: document.lineAt(position).text.substring(0, position.character),
        documentPrefix: document.getText(new vscode.Range(new vscode.Position(0, 0), position)),
        triggerKind: vsContext?.triggerKind ?? vscode.CompletionTriggerKind.Invoke,
        filePath: document.uri.fsPath,
        rootPath,
        get drafts() {
            drafts ??= unsavedDrafts(rootPath);
            return drafts;
        },
        getEnvironment: () => environmentProvider?.getSelectedEnvironment(),
    };
}

export async function listVariablesWithFallback(ctx: CompletionContext): Promise<vscode.CompletionItem[]> {
    try {
        const raw = await rqClient.listVariables(ctx.filePath, ctx.getEnvironment(), ctx.rootPath, ctx.drafts);
        const variables = filterRequiredVars(raw, ctx.documentPrefix, ctx.filePath);
        if (variables.length > 0) {
            return variables.map(v => {
                const item = new vscode.CompletionItem(v.name, vscode.CompletionItemKind.Variable);
                item.detail = v.value ? `= ${v.value}` : v.source;
                item.insertText = v.name;
                return item;
            });
        }
    } catch { /* fall through */ }
    const { parseVariables } = await import('./definitions');
    return parseVariables(ctx.document).map(v => {
        const item = new vscode.CompletionItem(v.name, vscode.CompletionItemKind.Variable);
        item.detail = `Variable (line ${v.line + 1})`;
        item.insertText = v.name;
        return item;
    });
}
