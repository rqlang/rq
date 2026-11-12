import * as vscode from 'vscode';
import * as cliService from '../../src/rqClient';
import { registerOpenRequestFileCommand } from '../../src/commands/openRequestFile';
import { RequestExplorerProvider, RequestTreeItem } from '../../src/requestExplorer';

jest.mock('../../src/rqClient');
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

        (cliService.showRequestLocation as jest.Mock).mockResolvedValue({
            file: '/path/to/file.rq',
            line: 1,
            character: 0
        });

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

    const item = { request: { name: 'login', file: '/tmp/project/b.rq' } } as unknown as RequestTreeItem;

    test('opens file and highlights request', async () => {
        await commandCallback(item);

        expect(vscode.workspace.openTextDocument).toHaveBeenCalledWith('/path/to/file.rq');
        expect(vscode.window.showTextDocument).toHaveBeenCalledWith(mockDocument);
        expect(mockEditor.revealRange).toHaveBeenCalled();
        expect(mockEditor.selection).toBeDefined();
    });

    test('locates the request in the file the item is listed under', async () => {
        await commandCallback(item);

        expect(cliService.showRequestLocation).toHaveBeenCalledWith('login', undefined, '/tmp/project/b.rq');
    });

    test('shows an error when called without a request item', async () => {
        await commandCallback();

        expect(vscode.window.showErrorMessage).toHaveBeenCalledWith('Cannot open: No request information available');
        expect(cliService.showRequestLocation).not.toHaveBeenCalled();
    });

    test('handles file open error', async () => {
        (vscode.workspace.openTextDocument as jest.Mock).mockRejectedValue(new Error('File not found'));

        await commandCallback(item);

        expect(vscode.window.showErrorMessage).toHaveBeenCalledWith('File not found');
    });

    test('does not highlight if showRequest fails', async () => {
        (cliService.showRequestLocation as jest.Mock).mockRejectedValue(new Error('Request not found'));

        await commandCallback(item);

        expect(vscode.window.showErrorMessage).toHaveBeenCalledWith('Request not found');
        expect(vscode.workspace.openTextDocument).not.toHaveBeenCalled();
    });

    test('sets and clears loading state', async () => {
        await commandCallback(item);

        expect(mockProvider.setItemLoading).toHaveBeenCalledWith(item, true);
        expect(mockProvider.setItemLoading).toHaveBeenCalledWith(item, false);
        expect(mockProvider.setItemLoading).toHaveBeenCalledTimes(2);
    });

    test('clears loading state even when request fails', async () => {
        (cliService.showRequestLocation as jest.Mock).mockRejectedValue(new Error('Request not found'));

        await commandCallback(item);

        expect(mockProvider.setItemLoading).toHaveBeenCalledWith(item, false);
    });

    test('is hidden from the command palette', () => {
        const manifest = require('../../package.json');

        expect(manifest.contributes.menus.commandPalette).toContainEqual({ command: 'rq.openRequestFile', when: 'false' });
    });
});
