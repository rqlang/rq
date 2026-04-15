import * as vscode from 'vscode';
import { registerClearOAuthCacheCommand } from '../../src/commands/clearOAuthCache';

describe('clearOAuthCache Command', () => {
    let context: vscode.ExtensionContext;
    let commandCallback: Function;
    let mockSecrets: Map<string, string>;

    beforeEach(() => {
        jest.clearAllMocks();
        mockSecrets = new Map();

        // Populate secrets with some dummy cache
        mockSecrets.set('oauth-token-cache', JSON.stringify([['key', { accessToken: 'abc' }]]));

        // Mock context with secrets storage
        context = {
            subscriptions: [],
            secrets: {
                get: jest.fn((key) => Promise.resolve(mockSecrets.get(key))),
                store: jest.fn((key, value) => {
                    mockSecrets.set(key, value);
                    return Promise.resolve();
                }),
                delete: jest.fn((key) => {
                    mockSecrets.delete(key);
                    return Promise.resolve();
                }),
                onDidChange: new vscode.EventEmitter().event
            }
        } as unknown as vscode.ExtensionContext;

        // Capture the command callback
        (vscode.commands.registerCommand as jest.Mock).mockImplementation((command, callback) => {
            if (command === 'rq.clearOAuthCache') {
                commandCallback = callback;
            }
            return { dispose: jest.fn() };
        });
    });

    test('registers command correctly', () => {
        registerClearOAuthCacheCommand(context);
        expect(vscode.commands.registerCommand).toHaveBeenCalledWith('rq.clearOAuthCache', expect.any(Function));
        expect(context.subscriptions.length).toBeGreaterThan(0);
    });

    test('clears secrets when executed', async () => {
        registerClearOAuthCacheCommand(context);
        
        // Execute the command
        await commandCallback();

        // Verify secrets.delete was called
        expect(context.secrets.delete).toHaveBeenCalledWith('oauth-token-cache');
        expect(mockSecrets.has('oauth-token-cache')).toBeFalsy();
        
        // Verify success message
        expect(vscode.window.showInformationMessage).toHaveBeenCalledWith(expect.stringContaining('successfully'));
    });

    test('handles errors gracefully', async () => {
        registerClearOAuthCacheCommand(context);

        // Make delete fail
        (context.secrets.delete as jest.Mock).mockRejectedValueOnce(new Error('Storage failure'));

        // Execute command
        await commandCallback();

        // Verify error message
        expect(vscode.window.showErrorMessage).toHaveBeenCalledWith(expect.stringContaining('Storage failure'));
    });
});
