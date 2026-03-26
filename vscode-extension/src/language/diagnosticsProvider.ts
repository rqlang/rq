import * as vscode from 'vscode';
import * as fs from 'fs';
import * as path from 'path';
import * as os from 'os';
import * as cliService from '../cliService';
import { normalizePath } from '../utils';

const DEBOUNCE_MS = 500;

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

        if (openDocuments.size > 0) {
            const tempRoot = this.rebuildTempRoot(folderPath);
            this.mirrorWorkspaceToTemp(folderPath, tempRoot, openDocuments);
            const result = await cliService.checkFolder(tempRoot, env);
            this.applyDiagnostics(result, tempRoot, folderPath);
        } else {
            this.clearTempRoot(folderPath);
            const result = await cliService.checkFolder(folderPath, env);
            this.applyDiagnostics(result, folderPath, folderPath);
        }
    }

    private rebuildTempRoot(folderPath: string): string {
        const key = normalizePath(folderPath);
        this.clearTempRoot(folderPath);
        const tempRoot = normalizePath(fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'rq-check-'))));
        this.tempRoots.set(key, tempRoot);
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

    private mirrorWorkspaceToTemp(
        folderPath: string,
        tempRoot: string,
        overrides: Map<string, string>
    ): void {
        const rqFiles = this.collectRqFiles(folderPath);
        for (const filePath of rqFiles) {
            const relative = path.relative(folderPath, filePath);
            const dest = path.join(tempRoot, relative);
            fs.mkdirSync(path.dirname(dest), { recursive: true });
            const override = overrides.get(normalizePath(filePath));
            if (override !== undefined) {
                fs.writeFileSync(dest, override, 'utf8');
            } else {
                fs.copyFileSync(filePath, dest);
            }
        }
    }

    private collectRqFiles(dir: string): string[] {
        const results: string[] = [];
        try {
            for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
                const full = path.join(dir, entry.name);
                if (entry.isDirectory()) {
                    results.push(...this.collectRqFiles(full));
                } else if (entry.isFile() && entry.name.endsWith('.rq')) {
                    results.push(full);
                }
            }
        } catch {
            // skip unreadable dirs
        }
        return results;
    }

    private applyDiagnostics(
        result: cliService.CheckResult,
        sourcePath: string,
        realPath: string
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

        const affectedFolderUris = new Set<string>();
        for (const key of diagnosticsMap.keys()) {
            const uri = vscode.Uri.file(key);
            this.diagnosticCollection.set(uri, diagnosticsMap.get(key)!);
            affectedFolderUris.add(key);
        }

        for (const [uriStr] of this.diagnosticCollection) {
            const filePath = uriStr.fsPath;
            if (!diagnosticsMap.has(normalizePath(filePath))) {
                this.diagnosticCollection.delete(uriStr);
            }
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
