import * as vscode from 'vscode';
import { registerRqFileWatcher } from '../src/rqFileWatcher';

describe('registerRqFileWatcher', () => {
    let context: vscode.ExtensionContext;
    let watcherEvents: Record<string, () => void>;
    let workspaceEvents: Record<string, (event: unknown) => void>;

    beforeEach(() => {
        jest.useFakeTimers();
        context = { subscriptions: [] } as unknown as vscode.ExtensionContext;
        watcherEvents = {};
        workspaceEvents = {};
        const capture = (events: Record<string, (event: unknown) => void>, name: string) => (listener: (event: unknown) => void) => {
            events[name] = listener;
            return { dispose: jest.fn() };
        };
        (vscode.workspace.createFileSystemWatcher as jest.Mock).mockReturnValue({
            onDidCreate: capture(watcherEvents, 'create'),
            onDidChange: capture(watcherEvents, 'change'),
            onDidDelete: capture(watcherEvents, 'delete'),
            dispose: jest.fn(),
        });
        (vscode.workspace.onDidDeleteFiles as jest.Mock).mockImplementation(capture(workspaceEvents, 'delete'));
        (vscode.workspace.onDidRenameFiles as jest.Mock).mockImplementation(capture(workspaceEvents, 'rename'));
    });

    afterEach(() => {
        jest.useRealTimers();
    });

    test('watches rq files', () => {
        registerRqFileWatcher(context, jest.fn());

        expect(vscode.workspace.createFileSystemWatcher).toHaveBeenCalledWith('**/*.rq');
    });

    test('notifies once for a burst of file events', () => {
        const onChange = jest.fn();
        registerRqFileWatcher(context, onChange);

        watcherEvents.create();
        watcherEvents.change();
        watcherEvents.delete();
        workspaceEvents.rename({ files: [{ oldUri: vscode.Uri.file('/root/a.rq'), newUri: vscode.Uri.file('/root/b.rq') }] });
        jest.runAllTimers();

        expect(onChange).toHaveBeenCalledTimes(1);
    });

    test('notifies when a folder is deleted from the workspace', () => {
        const onChange = jest.fn();
        registerRqFileWatcher(context, onChange);

        workspaceEvents.delete({ files: [vscode.Uri.file('/root/api')] });
        jest.runAllTimers();

        expect(onChange).toHaveBeenCalledTimes(1);
    });

    test('ignores deleting a file that is not an rq file', () => {
        const onChange = jest.fn();
        registerRqFileWatcher(context, onChange);

        workspaceEvents.delete({ files: [vscode.Uri.file('/root/logo.png')] });
        jest.runAllTimers();

        expect(onChange).not.toHaveBeenCalled();
    });

    test('ignores renaming a file that is not an rq file', () => {
        const onChange = jest.fn();
        registerRqFileWatcher(context, onChange);

        workspaceEvents.rename({ files: [{ oldUri: vscode.Uri.file('/root/a.png'), newUri: vscode.Uri.file('/root/b.png') }] });
        jest.runAllTimers();

        expect(onChange).not.toHaveBeenCalled();
    });

    test('does not notify after disposal', () => {
        const onChange = jest.fn();
        registerRqFileWatcher(context, onChange);

        watcherEvents.change();
        context.subscriptions.forEach(s => s.dispose());
        jest.runAllTimers();

        expect(onChange).not.toHaveBeenCalled();
    });
});
