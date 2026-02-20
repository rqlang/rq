import * as vscode from 'vscode';
import { registerOpenRequestFileCommand } from '../../src/commands/openRequestFile';

describe('openRequestFile Command', () => {
    let context: vscode.ExtensionContext;
    let commandCallback: Function;
    let mockEditor: any;
    let mockDocument: any;

    beforeEach(() => {
        jest.clearAllMocks();

        // Mock context
        context = {
            subscriptions: []
        } as unknown as vscode.ExtensionContext;

        // Mock document and editor
        mockDocument = {
            getText: jest.fn().mockReturnValue('some content\nrq myRequest\nmore content'),
            positionAt: jest.fn().mockReturnValue({ line: 1, character: 0 })
        };
        
        mockEditor = {
            selection: null,
            revealRange: jest.fn()
        };

        (vscode.workspace.openTextDocument as jest.Mock).mockResolvedValue(mockDocument);
        (vscode.window.showTextDocument as jest.Mock).mockResolvedValue(mockEditor);

        // Capture the command callback
        (vscode.commands.registerCommand as jest.Mock).mockImplementation((command, callback) => {
            if (command === 'rq.openRequestFile') {
                commandCallback = callback;
            }
            return { dispose: jest.fn() };
        });

        // Register the command
        registerOpenRequestFileCommand(context);
    });

    test('registers the command', () => {
        expect(vscode.commands.registerCommand).toHaveBeenCalledWith('rq.openRequestFile', expect.any(Function));
        expect(context.subscriptions).toHaveLength(1);
    });

    test('opens file and highlights request', async () => {
        await commandCallback('/path/to/file.rq', 'myRequest');

        expect(vscode.workspace.openTextDocument).toHaveBeenCalledWith('/path/to/file.rq');
        expect(vscode.window.showTextDocument).toHaveBeenCalledWith(mockDocument);
        
        // Verify highlighting logic
        expect(mockDocument.getText).toHaveBeenCalled();
        expect(mockDocument.positionAt).toHaveBeenCalled();
        expect(mockEditor.revealRange).toHaveBeenCalled();
        expect(mockEditor.selection).toBeDefined();
    });

    test('handles file open error', async () => {
        (vscode.workspace.openTextDocument as jest.Mock).mockRejectedValue(new Error('File not found'));

        await commandCallback('/path/to/file.rq', 'myRequest');

        expect(vscode.window.showErrorMessage).toHaveBeenCalledWith('Failed to open file: File not found');
    });

    test('does not highlight if request not found', async () => {
        mockDocument.getText.mockReturnValue('some content\nno match here');
        
        await commandCallback('/path/to/file.rq', 'myRequest');

        expect(mockEditor.revealRange).not.toHaveBeenCalled();
    });
});
