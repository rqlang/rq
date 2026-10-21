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
        vscode.workspace.onDidDeleteFiles(schedule),
        vscode.workspace.onDidRenameFiles(async ({ files }) => {
            for (const { oldUri, newUri } of files) {
                if (isRqFile(oldUri) || isRqFile(newUri) || await mayBeDirectory(newUri)) {
                    schedule();
                    return;
                }
            }
        }),
        { dispose: () => clearTimeout(timer) }
    );
}

function isRqFile(uri: vscode.Uri): boolean {
    return path.extname(uri.fsPath) === '.rq';
}

async function mayBeDirectory(uri: vscode.Uri): Promise<boolean> {
    try {
        return ((await vscode.workspace.fs.stat(uri)).type & vscode.FileType.Directory) !== 0;
    } catch {
        return true;
    }
}
