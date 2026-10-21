import * as vscode from 'vscode';
import { registerRqFileWatcher } from '../src/rqFileWatcher';

describe('registerRqFileWatcher', () => {
    let context: vscode.ExtensionContext;
    let watcherEvents: Record<string, () => void>;
    let workspaceEvents: Record<string, () => void>;

    beforeEach(() => {
        jest.useFakeTimers();
        context = { subscriptions: [] } as unknown as vscode.ExtensionContext;
        watcherEvents = {};
        workspaceEvents = {};
        const capture = (events: Record<string, () => void>, name: string) => (listener: () => void) => {
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
        workspaceEvents.rename();
        jest.runAllTimers();

        expect(onChange).toHaveBeenCalledTimes(1);
    });

    test('notifies when a folder is deleted from the workspace', () => {
        const onChange = jest.fn();
        registerRqFileWatcher(context, onChange);

        workspaceEvents.delete();
        jest.runAllTimers();

        expect(onChange).toHaveBeenCalledTimes(1);
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
