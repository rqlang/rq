import * as vscode from 'vscode';
import { registerSelectEnvironmentCommand } from '../../src/commands/selectEnvironment';
import { RequestExplorerProvider } from '../../src/requestExplorer';
import { createExplorerState } from '../memento-mock';
import * as cliService from '../../src/rqClient';
import { mockQuickPick } from '../quickPick-mock';

// Mock dependencies
jest.mock('../../src/requestExplorer');
jest.mock('../../src/rqClient');

describe('selectEnvironment Command', () => {
    let context: vscode.ExtensionContext;
    let provider: RequestExplorerProvider;
    let commandCallback: Function;

    beforeEach(() => {
        jest.clearAllMocks();
        (vscode.window.createQuickPick as jest.Mock).mockReset();
        (cliService.listEnvironments as jest.Mock).mockReset();

        // Mock context
        context = {
            subscriptions: []
        } as unknown as vscode.ExtensionContext;

        // Mock provider
        provider = new RequestExplorerProvider('root', createExplorerState());
        provider.setSelectedEnvironment = jest.fn();
        provider.getSelectedEnvironment = jest.fn().mockReturnValue(undefined);

        // Mock cliService defaults

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
        mockQuickPick({ accept: 'dev' });

        await commandCallback();

        expect(cliService.listEnvironments).toHaveBeenCalled();
        expect(vscode.window.createQuickPick).toHaveBeenCalled();
        expect(provider.setSelectedEnvironment).toHaveBeenCalledWith('dev');
        expect(vscode.window.showInformationMessage).toHaveBeenCalledWith('Environment set to: dev');
    });

    test('selects "None" environment', async () => {
        (cliService.listEnvironments as jest.Mock).mockResolvedValue(['dev']);
        mockQuickPick({ accept: 'None' });

        await commandCallback();

        expect(provider.setSelectedEnvironment).toHaveBeenCalledWith(undefined);
        expect(vscode.window.showInformationMessage).toHaveBeenCalledWith('No environment selected');
    });

    test('cancels selection', async () => {
        (cliService.listEnvironments as jest.Mock).mockResolvedValue(['dev']);
        mockQuickPick('cancel');

        await commandCallback();

        expect(provider.setSelectedEnvironment).not.toHaveBeenCalled();
    });

    test('preselects the current environment', async () => {
        (provider.getSelectedEnvironment as jest.Mock).mockReturnValue('prod');
        (cliService.listEnvironments as jest.Mock).mockResolvedValue(['dev', 'prod']);
        const target = mockQuickPick('cancel');

        await commandCallback();

        expect(target.activeItems.map(item => item.label)).toEqual(['prod']);
    });

    test('keeps an environment named None distinct from no environment', async () => {
        (cliService.listEnvironments as jest.Mock).mockResolvedValue(['None']);
        const target = mockQuickPick('cancel');
        (provider.getSelectedEnvironment as jest.Mock).mockReturnValue('None');

        await commandCallback();

        expect(target.items.indexOf(target.activeItems[0])).toBe(1);
    });

    test('handles error', async () => {
        (cliService.listEnvironments as jest.Mock).mockRejectedValue(new Error('CLI Error'));

        await commandCallback();

        expect(vscode.window.showErrorMessage).toHaveBeenCalledWith('Failed to select environment: CLI Error');
    });
});
