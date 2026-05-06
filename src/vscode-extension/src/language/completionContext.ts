import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import * as rqClient from '../rqClient';
import { normalizePath, mirrorToTemp } from '../utils';
import { filterRequiredVars } from './completionHelpers';

export interface CompletionContext {
    document: vscode.TextDocument;
    position: vscode.Position;
    linePrefix: string;
    documentPrefix: string;
    triggerKind: vscode.CompletionTriggerKind;
    getCliFilePath(): Promise<string>;
    getEnvironment(): string | undefined;
}

export type EnvironmentProvider = { getSelectedEnvironment(): string | undefined };

async function resolveCliPath(document: vscode.TextDocument): Promise<{ filePath: string; tempDir: string | null }> {
    const workspaceRoot = vscode.workspace.getWorkspaceFolder(document.uri)?.uri.fsPath
        ?? path.dirname(document.uri.fsPath);
    const overrides = new Map<string, string>();
    for (const doc of vscode.workspace.textDocuments) {
        if (doc.languageId === 'rq' && doc.isDirty && normalizePath(doc.uri.fsPath).startsWith(normalizePath(workspaceRoot))) {
            overrides.set(normalizePath(doc.uri.fsPath), doc.getText());
        }
    }
    if (overrides.size === 0) {
        return { filePath: document.uri.fsPath, tempDir: null };
    }
    const tempDir = mirrorToTemp(workspaceRoot, overrides);
    const relPath = path.relative(workspaceRoot, document.uri.fsPath);
    if (relPath.startsWith('..')) {
        return { filePath: document.uri.fsPath, tempDir };
    }
    return { filePath: path.join(tempDir, relPath), tempDir };
}

export function buildContext(
    document: vscode.TextDocument,
    position: vscode.Position,
    vsContext: vscode.CompletionContext | undefined,
    environmentProvider: EnvironmentProvider | undefined
): { ctx: CompletionContext; cleanup: () => void } {
    let cliPathResult: { filePath: string; tempDir: string | null } | undefined;

    const getCliFilePath = async (): Promise<string> => {
        if (!cliPathResult) {
            try {
                cliPathResult = await resolveCliPath(document);
            } catch {
                cliPathResult = { filePath: document.uri.fsPath, tempDir: null };
            }
        }
        return cliPathResult.filePath;
    };

    const ctx: CompletionContext = {
        document,
        position,
        linePrefix: document.lineAt(position).text.substring(0, position.character),
        documentPrefix: document.getText(new vscode.Range(new vscode.Position(0, 0), position)),
        triggerKind: vsContext?.triggerKind ?? vscode.CompletionTriggerKind.Invoke,
        getCliFilePath,
        getEnvironment: () => environmentProvider?.getSelectedEnvironment(),
    };

    const cleanup = () => {
        if (cliPathResult?.tempDir) {
            fs.rmSync(cliPathResult.tempDir, { recursive: true, force: true });
        }
    };

    return { ctx, cleanup };
}

export async function listVariablesWithFallback(ctx: CompletionContext): Promise<vscode.CompletionItem[]> {
    try {
        const raw = await rqClient.listVariables(await ctx.getCliFilePath(), ctx.getEnvironment());
        const variables = filterRequiredVars(raw, ctx.documentPrefix);
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
