import * as vscode from 'vscode';
import * as path from 'path';

const DEBOUNCE_MS = 300;

export function registerRqFileWatcher(context: vscode.ExtensionContext, onChange: () => void): void {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const schedule = () => {
        clearTimeout(timer);
        timer = setTimeout(onChange, DEBOUNCE_MS);
    };

    const watcher = vscode.workspace.createFileSystemWatcher('**/*.rq');
    context.subscriptions.push(
        watcher,
        watcher.onDidCreate(schedule),
        watcher.onDidChange(schedule),
        watcher.onDidDelete(schedule),
        vscode.workspace.onDidDeleteFiles(({ files }) => {
            if (files.some(mayContainRqFiles)) { schedule(); }
        }),
        vscode.workspace.onDidRenameFiles(({ files }) => {
            if (files.some(({ oldUri, newUri }) => mayContainRqFiles(oldUri) || mayContainRqFiles(newUri))) { schedule(); }
        }),
        { dispose: () => clearTimeout(timer) }
    );
}

function mayContainRqFiles(uri: vscode.Uri): boolean {
    const extension = path.extname(uri.fsPath);
    return extension === '' || extension === '.rq';
}
