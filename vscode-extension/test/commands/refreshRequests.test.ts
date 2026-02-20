import * as vscode from 'vscode';
import { registerRefreshRequestsCommand } from '../../src/commands/refreshRequests';
import { RequestExplorerProvider } from '../../src/requestExplorer';
import * as cliService from '../../src/cliService';

// Mock dependencies
jest.mock('../../src/requestExplorer');

describe('refreshRequests Command', () => {
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
        provider.refresh = jest.fn();

        // Mock cliService
        jest.spyOn(cliService, 'isCliInstalling').mockReturnValue(false);
        jest.spyOn(cliService, 'isCliBinaryAvailable').mockReturnValue(false);
        jest.spyOn(cliService, 'handleCliNotFoundError').mockResolvedValue(undefined);

        // Capture the command callback
        (vscode.commands.registerCommand as jest.Mock).mockImplementation((command, callback) => {
            if (command === 'rq.refreshRequests') {
                commandCallback = callback;
            }
            return { dispose: jest.fn() };
        });

        // Register the command
        registerRefreshRequestsCommand(context, provider);
    });

    test('registers the command', () => {
        expect(vscode.commands.registerCommand).toHaveBeenCalledWith('rq.refreshRequests', expect.any(Function));
        expect(context.subscriptions).toHaveLength(1);
    });

    test('calls provider.refresh() when executed', async () => {
        await commandCallback();
        expect(provider.refresh).toHaveBeenCalled();
    });
});
