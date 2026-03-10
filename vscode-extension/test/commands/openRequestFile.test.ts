import * as vscode from 'vscode';
import * as cliService from '../../src/cliService';
import { registerOpenRequestFileCommand } from '../../src/commands/openRequestFile';
import { RequestExplorerProvider } from '../../src/requestExplorer';

jest.mock('../../src/cliService');
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

        mockDocument = {
            getText: jest.fn().mockReturnValue('some content\nrq myRequest\nmore content'),
            positionAt: jest.fn().mockReturnValue({ line: 1, character: 0 })
        };

        mockEditor = {
            selection: null,
            revealRange: jest.fn()
        };

        (cliService.showRequest as jest.Mock).mockResolvedValue({
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

    test('opens file and highlights request', async () => {
        await commandCallback('myRequest');

        expect(cliService.showRequest).toHaveBeenCalledWith('myRequest', undefined);
        expect(vscode.workspace.openTextDocument).toHaveBeenCalledWith('/path/to/file.rq');
        expect(vscode.window.showTextDocument).toHaveBeenCalledWith(mockDocument);
        expect(mockEditor.revealRange).toHaveBeenCalled();
        expect(mockEditor.selection).toBeDefined();
    });

    test('handles file open error', async () => {
        (vscode.workspace.openTextDocument as jest.Mock).mockRejectedValue(new Error('File not found'));

        await commandCallback('myRequest');

        expect(vscode.window.showErrorMessage).toHaveBeenCalledWith('Failed to open file: File not found');
    });

    test('does not highlight if showRequest fails', async () => {
        (cliService.showRequest as jest.Mock).mockRejectedValue(new Error('Request not found'));

        await commandCallback('myRequest');

        expect(vscode.window.showErrorMessage).toHaveBeenCalledWith('Failed to open file: Request not found');
        expect(vscode.workspace.openTextDocument).not.toHaveBeenCalled();
    });
});
