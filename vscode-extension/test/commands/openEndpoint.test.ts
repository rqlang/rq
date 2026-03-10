import * as vscode from 'vscode';
import { registerOpenEndpointCommand } from '../../src/commands/openEndpoint';
import { RequestExplorerProvider, RequestTreeItem } from '../../src/requestExplorer';

jest.mock('../../src/requestExplorer');

describe('openEndpoint Command', () => {
    let context: vscode.ExtensionContext;
    let commandCallback: Function;
    let mockEditor: any;
    let mockDocument: any;
    let mockProvider: jest.Mocked<RequestExplorerProvider>;

    beforeEach(() => {
        jest.clearAllMocks();

        context = {
            subscriptions: []
        } as unknown as vscode.ExtensionContext;

        mockProvider = {
            setItemLoading: jest.fn()
        } as unknown as jest.Mocked<RequestExplorerProvider>;

        mockDocument = {};

        mockEditor = {
            selection: null,
            revealRange: jest.fn()
        };

        (vscode.workspace.openTextDocument as jest.Mock).mockResolvedValue(mockDocument);
        (vscode.window.showTextDocument as jest.Mock).mockResolvedValue(mockEditor);

        (vscode.commands.registerCommand as jest.Mock).mockImplementation((command, callback) => {
            if (command === 'rq.openEndpoint') {
                commandCallback = callback;
            }
            return { dispose: jest.fn() };
        });

        registerOpenEndpointCommand(context, mockProvider);
    });

    test('registers the command', () => {
        expect(vscode.commands.registerCommand).toHaveBeenCalledWith('rq.openEndpoint', expect.any(Function));
        expect(context.subscriptions).toHaveLength(1);
    });

    test('opens file and navigates to position', async () => {
        await commandCallback('/root/api.rq', 5, 0);

        expect(vscode.workspace.openTextDocument).toHaveBeenCalledWith('/root/api.rq');
        expect(vscode.window.showTextDocument).toHaveBeenCalledWith(mockDocument);
        expect(mockEditor.revealRange).toHaveBeenCalled();
        expect(mockEditor.selection).toBeDefined();
    });

    test('handles file open error', async () => {
        (vscode.workspace.openTextDocument as jest.Mock).mockRejectedValue(new Error('File not found'));

        await commandCallback('/root/api.rq', 5, 0);

        expect(vscode.window.showErrorMessage).toHaveBeenCalledWith('Failed to open file: File not found');
    });

    test('sets and clears loading state when item is provided', async () => {
        const item = new RequestTreeItem('ep', null, vscode.TreeItemCollapsibleState.None);

        await commandCallback('/root/api.rq', 5, 0, item);

        expect(mockProvider.setItemLoading).toHaveBeenCalledWith(item, true);
        expect(mockProvider.setItemLoading).toHaveBeenCalledWith(item, false);
        expect(mockProvider.setItemLoading).toHaveBeenCalledTimes(2);
    });

    test('clears loading state even when file open fails', async () => {
        (vscode.workspace.openTextDocument as jest.Mock).mockRejectedValue(new Error('File not found'));
        const item = new RequestTreeItem('ep', null, vscode.TreeItemCollapsibleState.None);

        await commandCallback('/root/api.rq', 5, 0, item);

        expect(mockProvider.setItemLoading).toHaveBeenCalledWith(item, true);
        expect(mockProvider.setItemLoading).toHaveBeenCalledWith(item, false);
    });

    test('does not call setItemLoading when no item is provided', async () => {
        await commandCallback('/root/api.rq', 5, 0);

        expect(mockProvider.setItemLoading).not.toHaveBeenCalled();
    });
});
