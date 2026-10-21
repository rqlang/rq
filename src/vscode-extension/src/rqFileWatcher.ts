import * as vscode from 'vscode';

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
        vscode.workspace.onDidDeleteFiles(schedule),
        vscode.workspace.onDidRenameFiles(schedule),
        { dispose: () => clearTimeout(timer) }
    );
}
