import * as vscode from 'vscode';
import * as fs from 'fs';
import * as path from 'path';
import * as rqClient from '../rqClient';
import { normalizePath, mirrorToTemp } from '../utils';

const DEBOUNCE_MS = 1000;
const LINT_SETTING = 'rq.lint.enabled';

interface EnvironmentProvider {
    getSelectedEnvironment(): string | undefined;
}

export class DiagnosticsProvider {
    private readonly diagnosticCollection: vscode.DiagnosticCollection;
    private readonly environmentProvider: EnvironmentProvider | undefined;
    private readonly tempRoots = new Map<string, string>();
    private readonly debounceTimers = new Map<string, ReturnType<typeof setTimeout>>();

    constructor(diagnosticCollection: vscode.DiagnosticCollection, environmentProvider?: EnvironmentProvider) {
        this.diagnosticCollection = diagnosticCollection;
        this.environmentProvider = environmentProvider;
    }

    scheduleValidation(document: vscode.TextDocument): void {
        if (document.languageId !== 'rq') {
            return;
        }

        const workspaceFolder = vscode.workspace.getWorkspaceFolder(document.uri);
        if (!workspaceFolder) {
            return;
        }

        const folderKey = normalizePath(workspaceFolder.uri.fsPath);
        const existing = this.debounceTimers.get(folderKey);
        if (existing) {
            clearTimeout(existing);
        }

        const timer = setTimeout(() => {
            this.debounceTimers.delete(folderKey);
            void this.validateFolder(workspaceFolder);
        }, DEBOUNCE_MS);

        this.debounceTimers.set(folderKey, timer);
    }

    validateAllFolders(): void {
        for (const folder of vscode.workspace.workspaceFolders ?? []) {
            void this.validateFolder(folder);
        }
    }

    validateSaved(document: vscode.TextDocument): void {
        if (document.languageId !== 'rq') {
            return;
        }

        const workspaceFolder = vscode.workspace.getWorkspaceFolder(document.uri);
        if (!workspaceFolder) {
            return;
        }

        const folderKey = normalizePath(workspaceFolder.uri.fsPath);
        const existing = this.debounceTimers.get(folderKey);
        if (existing) {
            clearTimeout(existing);
            this.debounceTimers.delete(folderKey);
        }

        void this.validateFolder(workspaceFolder);
    }

    dispose(): void {
        for (const timer of this.debounceTimers.values()) {
            clearTimeout(timer);
        }
        this.debounceTimers.clear();
        for (const tempRoot of this.tempRoots.values()) {
            this.removeTempDir(tempRoot);
        }
        this.tempRoots.clear();
    }

    private async validateFolder(workspaceFolder: vscode.WorkspaceFolder): Promise<void> {
        const folderPath = workspaceFolder.uri.fsPath;
        const openDocuments = new Map<string, string>();

        for (const doc of vscode.workspace.textDocuments) {
            if (doc.languageId === 'rq' && doc.isDirty) {
                const wsFolder = vscode.workspace.getWorkspaceFolder(doc.uri);
                if (wsFolder && normalizePath(wsFolder.uri.fsPath) === normalizePath(folderPath)) {
                    openDocuments.set(normalizePath(doc.uri.fsPath), doc.getText());
                }
            }
        }

        const env = this.environmentProvider?.getSelectedEnvironment();

        const lint = await this.collectLintDiagnostics(folderPath);

        if (openDocuments.size > 0) {
            const tempRoot = this.rebuildTempRoot(folderPath, openDocuments);
            const result = await rqClient.checkFolder(tempRoot, env);
            this.applyDiagnostics(result, tempRoot, folderPath, lint);
        } else {
            this.clearTempRoot(folderPath);
            const result = await rqClient.checkFolder(folderPath, env);
            this.applyDiagnostics(result, folderPath, folderPath, lint);
        }
    }

    private lintEnabled(): boolean {
        const [section, key] = [LINT_SETTING.slice(0, LINT_SETTING.lastIndexOf('.')), LINT_SETTING.slice(LINT_SETTING.lastIndexOf('.') + 1)];
        return vscode.workspace.getConfiguration(section).get<boolean>(key, true);
    }

    private async collectLintDiagnostics(folderPath: string): Promise<Map<string, vscode.Diagnostic[]>> {
        const byFile = new Map<string, vscode.Diagnostic[]>();
        if (!this.lintEnabled()) {
            return byFile;
        }

        const drafts = vscode.workspace.textDocuments
            .filter(doc => doc.languageId === 'rq')
            .filter(doc => {
                const wsFolder = vscode.workspace.getWorkspaceFolder(doc.uri);
                return wsFolder !== undefined && normalizePath(wsFolder.uri.fsPath) === normalizePath(folderPath);
            })
            .map(doc => ({ path: doc.uri.fsPath, source: doc.getText() }));

        if (drafts.length === 0) {
            return byFile;
        }

        let results: Map<string, rqClient.LintResult>;
        try {
            results = await rqClient.lintSources(drafts, folderPath);
        } catch {
            return byFile;
        }

        for (const draft of drafts) {
            const result = results.get(normalizePath(draft.path).replace(/\\/g, '/'));
            if (!result || result.diagnostics.length === 0) {
                continue;
            }
            byFile.set(normalizePath(draft.path), result.diagnostics.map(toLintDiagnostic));
        }
        return byFile;
    }

    private rebuildTempRoot(folderPath: string, overrides: Map<string, string>): string {
        this.clearTempRoot(folderPath);
        const tempRoot = mirrorToTemp(folderPath, overrides);
        this.tempRoots.set(normalizePath(folderPath), tempRoot);
        return tempRoot;
    }

    private clearTempRoot(folderPath: string): void {
        const key = normalizePath(folderPath);
        const existing = this.tempRoots.get(key);
        if (existing) {
            this.removeTempDir(existing);
            this.tempRoots.delete(key);
        }
    }

    private applyDiagnostics(
        result: rqClient.CheckResult,
        sourcePath: string,
        realPath: string,
        lintDiagnostics: Map<string, vscode.Diagnostic[]> = new Map()
    ): void {
        const diagnosticsMap = new Map<string, vscode.Diagnostic[]>();

        for (const e of result.errors) {
            const normalizedSource = normalizePath(sourcePath);
            const normalizedReal = normalizePath(realPath);
            const normalizedFile = normalizePath(e.file);
            const realFile = normalizedSource === normalizedReal
                ? normalizedFile
                : normalizePath(path.join(normalizedReal, path.relative(normalizedSource, normalizedFile)));
            const range = new vscode.Range(e.line - 1, e.column - 1, e.line - 1, Number.MAX_VALUE);
            const diagnostic = new vscode.Diagnostic(range, e.message, vscode.DiagnosticSeverity.Error);
            diagnostic.source = 'rq';

            if (!diagnosticsMap.has(realFile)) {
                diagnosticsMap.set(realFile, []);
            }
            diagnosticsMap.get(realFile)!.push(diagnostic);
        }

        for (const [file, diagnostics] of lintDiagnostics) {
            const existing = diagnosticsMap.get(file);
            if (existing) {
                existing.push(...diagnostics);
            } else {
                diagnosticsMap.set(file, [...diagnostics]);
            }
        }

        const affectedFolderUris = new Set<string>();
        for (const key of diagnosticsMap.keys()) {
            const uri = vscode.Uri.file(key);
            this.diagnosticCollection.set(uri, diagnosticsMap.get(key)!);
            affectedFolderUris.add(key);
        }

        const toDelete: vscode.Uri[] = [];
        for (const [uri] of this.diagnosticCollection) {
            const filePath = normalizePath(uri.fsPath);
            if (filePath.startsWith(normalizePath(realPath)) && !diagnosticsMap.has(filePath)) {
                toDelete.push(uri);
            }
        }
        for (const uri of toDelete) {
            this.diagnosticCollection.delete(uri);
        }
    }

    private removeTempDir(tempRoot: string): void {
        try {
            fs.rmSync(tempRoot, { recursive: true, force: true });
        } catch {
            // ignore
        }
    }
}

function toLintDiagnostic(entry: rqClient.LintDiagnostic): vscode.Diagnostic {
    const line = Math.max(0, entry.line - 1);
    const column = Math.max(0, entry.column - 1);
    const range = new vscode.Range(line, column, line, Number.MAX_VALUE);
    const message = entry.suggested_fix
        ? `${entry.message}\n\nSuggested fix: ${entry.suggested_fix}`
        : entry.message;
    const diagnostic = new vscode.Diagnostic(range, message, vscode.DiagnosticSeverity.Warning);
    diagnostic.source = 'rq lint';
    diagnostic.code = entry.rule;
    return diagnostic;
}
