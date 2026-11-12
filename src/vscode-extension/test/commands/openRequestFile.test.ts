import * as vscode from 'vscode';
import { registerOpenRequestFileCommand } from '../../src/commands/openRequestFile';
import { RequestExplorerProvider, RequestTreeItem } from '../../src/requestExplorer';

jest.mock('../../src/requestExplorer');

describe('openRequestFile Command', () => {
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
            if (command === 'rq.openRequestFile') {
                commandCallback = callback;
            }
            return { dispose: jest.fn() };
        });

        registerOpenRequestFileCommand(context, mockProvider);
    });

    test('registers the command', () => {
        expect(vscode.commands.registerCommand).toHaveBeenCalledWith('rq.openRequestFile', expect.any(Function));
        expect(context.subscriptions).toHaveLength(1);
    });

    const item = {
        request: { name: 'login', file: '/tmp/project/a.rq', request_file: '/tmp/project/b.rq', line: 3, character: 4 }
    } as unknown as RequestTreeItem;

    test('opens the file that declares the request', async () => {
        await commandCallback(item);

        expect(vscode.workspace.openTextDocument).toHaveBeenCalledWith('/tmp/project/b.rq');
        expect(vscode.window.showTextDocument).toHaveBeenCalledWith(mockDocument);
    });

    test('places the cursor at the listed position of the request', async () => {
        await commandCallback(item);

        expect(mockEditor.selection.start).toEqual(new vscode.Position(3, 4));
        expect(mockEditor.revealRange).toHaveBeenCalled();
    });

    test('shows an error when called without a request item', async () => {
        await commandCallback();

        expect(vscode.window.showErrorMessage).toHaveBeenCalledWith('Cannot open: No request information available');
        expect(vscode.workspace.openTextDocument).not.toHaveBeenCalled();
    });

    test('handles file open error', async () => {
        (vscode.workspace.openTextDocument as jest.Mock).mockRejectedValue(new Error('File not found'));

        await commandCallback(item);

        expect(vscode.window.showErrorMessage).toHaveBeenCalledWith('File not found');
    });

    test('sets and clears loading state', async () => {
        await commandCallback(item);

        expect(mockProvider.setItemLoading).toHaveBeenCalledWith(item, true);
        expect(mockProvider.setItemLoading).toHaveBeenCalledWith(item, false);
        expect(mockProvider.setItemLoading).toHaveBeenCalledTimes(2);
    });

    test('clears loading state even when the file cannot be opened', async () => {
        (vscode.workspace.openTextDocument as jest.Mock).mockRejectedValue(new Error('File not found'));

        await commandCallback(item);

        expect(mockProvider.setItemLoading).toHaveBeenCalledWith(item, false);
    });

    test('is hidden from the command palette', () => {
        const manifest = require('../../package.json');

        expect(manifest.contributes.menus.commandPalette).toContainEqual({ command: 'rq.openRequestFile', when: 'false' });
    });
});
