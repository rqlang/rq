import * as vscode from 'vscode';
import { registerSelectEnvironmentCommand } from '../../src/commands/selectEnvironment';
import { RequestExplorerProvider } from '../../src/requestExplorer';
import * as cliService from '../../src/cliService';

// Mock dependencies
jest.mock('../../src/requestExplorer');
jest.mock('../../src/cliService');

describe('selectEnvironment Command', () => {
    let context: vscode.ExtensionContext;
    let provider: RequestExplorerProvider;
    let commandCallback: Function;

    beforeEach(() => {
        jest.clearAllMocks();

        // Mock context
        context = {
            subscriptions: []
        } as unknown as vscode.ExtensionContext;

        // Mock provider
        provider = new RequestExplorerProvider('root');
        provider.setSelectedEnvironment = jest.fn();

        // Mock cliService defaults
        (cliService as any).isCliInstalling.mockReturnValue(false);
        (cliService as any).isCliBinaryAvailable.mockReturnValue(true);

        // Capture the command callback
        (vscode.commands.registerCommand as jest.Mock).mockImplementation((command, callback) => {
            if (command === 'rq.selectEnvironment') {
                commandCallback = callback;
            }
            return { dispose: jest.fn() };
        });

        // Register the command
        registerSelectEnvironmentCommand(context, provider);
    });

    test('registers the command', () => {
        expect(vscode.commands.registerCommand).toHaveBeenCalledWith('rq.selectEnvironment', expect.any(Function));
        expect(context.subscriptions).toHaveLength(1);
    });

    test('selects an environment', async () => {
        (cliService.listEnvironments as jest.Mock).mockResolvedValue(['dev', 'prod']);
        (vscode.window.showQuickPick as jest.Mock).mockResolvedValue({ label: 'dev' });

        await commandCallback();

        expect(cliService.listEnvironments).toHaveBeenCalled();
        expect(vscode.window.showQuickPick).toHaveBeenCalled();
        expect(provider.setSelectedEnvironment).toHaveBeenCalledWith('dev');
        expect(vscode.window.showInformationMessage).toHaveBeenCalledWith('Environment set to: dev');
    });

    test('selects "None" environment', async () => {
        (cliService.listEnvironments as jest.Mock).mockResolvedValue(['dev']);
        (vscode.window.showQuickPick as jest.Mock).mockResolvedValue({ label: 'None' });

        await commandCallback();

        expect(provider.setSelectedEnvironment).toHaveBeenCalledWith(undefined);
        expect(vscode.window.showInformationMessage).toHaveBeenCalledWith('No environment selected');
    });

    test('cancels selection', async () => {
        (cliService.listEnvironments as jest.Mock).mockResolvedValue(['dev']);
        (vscode.window.showQuickPick as jest.Mock).mockResolvedValue(undefined);

        await commandCallback();

        expect(provider.setSelectedEnvironment).not.toHaveBeenCalled();
    });

    test('handles error', async () => {
        (cliService.listEnvironments as jest.Mock).mockRejectedValue(new Error('CLI Error'));

        await commandCallback();

        expect(vscode.window.showErrorMessage).toHaveBeenCalledWith('Failed to select environment: CLI Error');
    });
});
