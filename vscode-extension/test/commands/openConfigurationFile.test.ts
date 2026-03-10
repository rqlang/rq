import * as vscode from 'vscode';
import * as cliService from '../../src/cliService';
import { registerOpenConfigurationFileCommand } from '../../src/commands/openConfigurationFile';
import { ConfigurationExplorerProvider, ConfigurationTreeItem } from '../../src/configurationExplorer';

jest.mock('../../src/cliService');
jest.mock('../../src/configurationExplorer');

describe('openConfigurationFile Command', () => {
    let context: vscode.ExtensionContext;
    let commandCallback: Function;
    let mockEditor: any;
    let mockDocument: any;
    let mockProvider: jest.Mocked<ConfigurationExplorerProvider>;

    beforeEach(() => {
        jest.clearAllMocks();

        context = {
            subscriptions: []
        } as unknown as vscode.ExtensionContext;

        mockProvider = {
            setItemLoading: jest.fn()
        } as unknown as jest.Mocked<ConfigurationExplorerProvider>;

        mockDocument = {};
        mockEditor = {
            selection: null,
            revealRange: jest.fn()
        };

        (vscode.workspace.openTextDocument as jest.Mock).mockResolvedValue(mockDocument);
        (vscode.window.showTextDocument as jest.Mock).mockResolvedValue(mockEditor);

        (vscode.commands.registerCommand as jest.Mock).mockImplementation((command, callback) => {
            if (command === 'rq.openConfigurationFile') {
                commandCallback = callback;
            }
            return { dispose: jest.fn() };
        });

        registerOpenConfigurationFileCommand(context, mockProvider);
    });

    test('registers the command', () => {
        expect(vscode.commands.registerCommand).toHaveBeenCalledWith('rq.openConfigurationFile', expect.any(Function));
        expect(context.subscriptions).toHaveLength(1);
    });

    test('opens file at correct position for env artifact', async () => {
        (cliService.showEnvironment as jest.Mock).mockResolvedValue({
            name: 'dev',
            file: '/path/to/env.rq',
            line: 3,
            character: 0
        });

        await commandCallback('env', 'dev');

        expect(cliService.showEnvironment).toHaveBeenCalledWith('dev', undefined);
        expect(vscode.workspace.openTextDocument).toHaveBeenCalledWith('/path/to/env.rq');
        expect(vscode.window.showTextDocument).toHaveBeenCalledWith(mockDocument);
        expect(mockEditor.revealRange).toHaveBeenCalled();
        expect(mockEditor.selection).toBeDefined();
    });

    test('opens file at correct position for auth artifact', async () => {
        (cliService.showAuthConfig as jest.Mock).mockResolvedValue({
            name: 'my_auth',
            auth_type: 'bearer',
            fields: {},
            file: '/path/to/auth.rq',
            line: 7,
            character: 0
        });

        await commandCallback('auth', 'my_auth');

        expect(cliService.showAuthConfig).toHaveBeenCalledWith('my_auth', undefined);
        expect(vscode.workspace.openTextDocument).toHaveBeenCalledWith('/path/to/auth.rq');
        expect(vscode.window.showTextDocument).toHaveBeenCalledWith(mockDocument);
        expect(mockEditor.revealRange).toHaveBeenCalled();
    });

    test('shows error message when env lookup fails', async () => {
        (cliService.showEnvironment as jest.Mock).mockRejectedValue(new Error('Environment not found'));

        await commandCallback('env', 'missing');

        expect(vscode.window.showErrorMessage).toHaveBeenCalledWith('Environment not found');
        expect(vscode.workspace.openTextDocument).not.toHaveBeenCalled();
    });

    test('shows error message when auth lookup fails', async () => {
        (cliService.showAuthConfig as jest.Mock).mockRejectedValue(new Error('Auth not found'));

        await commandCallback('auth', 'missing');

        expect(vscode.window.showErrorMessage).toHaveBeenCalledWith('Auth not found');
        expect(vscode.workspace.openTextDocument).not.toHaveBeenCalled();
    });

    test('shows error message when file open fails', async () => {
        (cliService.showEnvironment as jest.Mock).mockResolvedValue({
            name: 'dev',
            file: '/path/to/env.rq',
            line: 0,
            character: 0
        });
        (vscode.workspace.openTextDocument as jest.Mock).mockRejectedValue(new Error('File not found'));

        await commandCallback('env', 'dev');

        expect(vscode.window.showErrorMessage).toHaveBeenCalledWith('File not found');
    });

    test('sets and clears loading state when item is provided', async () => {
        (cliService.showEnvironment as jest.Mock).mockResolvedValue({
            name: 'dev',
            file: '/path/to/env.rq',
            line: 0,
            character: 0
        });
        const item = new ConfigurationTreeItem('dev', 'environment', vscode.TreeItemCollapsibleState.None);

        await commandCallback('env', 'dev', item);

        expect(mockProvider.setItemLoading).toHaveBeenCalledWith(item, true);
        expect(mockProvider.setItemLoading).toHaveBeenCalledWith(item, false);
        expect(mockProvider.setItemLoading).toHaveBeenCalledTimes(2);
    });

    test('clears loading state even when lookup fails', async () => {
        (cliService.showEnvironment as jest.Mock).mockRejectedValue(new Error('Environment not found'));
        const item = new ConfigurationTreeItem('missing', 'environment', vscode.TreeItemCollapsibleState.None);

        await commandCallback('env', 'missing', item);

        expect(mockProvider.setItemLoading).toHaveBeenCalledWith(item, true);
        expect(mockProvider.setItemLoading).toHaveBeenCalledWith(item, false);
    });

    test('does not call setItemLoading when no item is provided', async () => {
        (cliService.showEnvironment as jest.Mock).mockResolvedValue({
            name: 'dev',
            file: '/path/to/env.rq',
            line: 0,
            character: 0
        });

        await commandCallback('env', 'dev');

        expect(mockProvider.setItemLoading).not.toHaveBeenCalled();
    });
});
