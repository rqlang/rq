import * as vscode from 'vscode';
import { registerGetTokenCommand } from '../../src/commands/getToken';
import * as cliService from '../../src/rqClient';
import { FetchMock } from '../fetch-mock';
import { authUriHandler } from '../../src/auth/authUriHandler';

// Mock CLI service but NOT auth module
jest.mock('../../src/rqClient');

type FakeQuickPick = {
    items: { label: string }[];
    activeItems: { label: string }[];
    selectedItems: { label: string }[];
};

function mockEnvironmentPick(choice: string | undefined): FakeQuickPick {
    const acceptListeners: (() => void)[] = [];
    const hideListeners: (() => void)[] = [];
    const quickPick = {
        items: [] as { label: string }[],
        activeItems: [] as { label: string }[],
        selectedItems: [] as { label: string }[],
        onDidAccept: (listener: () => void) => { acceptListeners.push(listener); return { dispose: jest.fn() }; },
        onDidHide: (listener: () => void) => { hideListeners.push(listener); return { dispose: jest.fn() }; },
        hide: () => hideListeners.forEach(listener => listener()),
        dispose: jest.fn(),
        show: () => {
            if (choice === undefined) {
                quickPick.hide();
                return;
            }
            quickPick.selectedItems = quickPick.items.filter(item => item.label === choice);
            acceptListeners.forEach(listener => listener());
        }
    };
    (vscode.window.createQuickPick as jest.Mock).mockReturnValueOnce(quickPick);
    return quickPick;
}

describe('getToken Command (Integration)', () => {
    let context: vscode.ExtensionContext;
    let outputChannel: vscode.OutputChannel;
    let commandCallback: Function;
    let mockSecrets: Map<string, string>;
    let fetchMock: FetchMock;
    let currentEnvironment: string | undefined;

    beforeEach(() => {
        jest.clearAllMocks();
        mockSecrets = new Map();
        fetchMock = new FetchMock();
        currentEnvironment = undefined;

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

        // Mock output channel
        outputChannel = {
            appendLine: jest.fn(),
            show: jest.fn()
        } as unknown as vscode.OutputChannel;

        // Mock cliService defaults

        // Capture the command callback
        (vscode.commands.registerCommand as jest.Mock).mockImplementation((command, callback) => {
            if (command === 'rq.getToken') {
                commandCallback = callback;
            }
            return { dispose: jest.fn() };
        });

        // Register the command
        registerGetTokenCommand(context, outputChannel, { getSelectedEnvironment: () => currentEnvironment });
    });

    afterEach(() => {
        fetchMock.restore();
    });

    test('full OAuth2 flow with VS Code URI handler', async () => {
        // 1. Setup CLI mocks
        (cliService.listEnvironments as jest.Mock).mockResolvedValue(['dev']);
        (cliService.listAuthConfigs as jest.Mock).mockResolvedValue([
            { name: 'auth1', auth_type: 'oauth2_authorization_code' }
        ]);
        (cliService.showAuthConfig as jest.Mock).mockResolvedValue({
            name: 'auth1',
            auth_type: 'oauth2_authorization_code',
            fields: {
                client_id: 'test-client',
                authorization_url: 'https://auth.example.com/authorize',
                token_url: 'https://auth.example.com/token',
                redirect_uri: 'vscode://rq-lang.rq-language/callback',
                scope: 'api:read'
            }
        });

        // 2. Setup User Interactions
        mockEnvironmentPick('dev');
        (vscode.window.showQuickPick as jest.Mock)
            .mockResolvedValueOnce('auth1'); // Select auth config
        
        (vscode.window.showInformationMessage as jest.Mock)
            .mockResolvedValueOnce('Copy Token'); // Success message

        // 3. Setup Fetch Mock for Token Exchange
        fetchMock.mockResponse({
            access_token: 'real-integration-token',
            token_type: 'Bearer',
            expires_in: 3600,
            scope: 'api:read'
        });

        // 4. Start the command execution
        const commandPromise = commandCallback();

        // 5. Wait for openExternal to be called (browser opened)
        await new Promise(resolve => setTimeout(resolve, 100));
        expect(vscode.env.openExternal).toHaveBeenCalled();
        
        // Verify the URL opened contains correct params
        const openedUri = (vscode.env.openExternal as jest.Mock).mock.calls[0][0];
        const openedUrlString = openedUri.toString();
        expect(openedUrlString).toContain('https://auth.example.com/authorize');
        expect(openedUrlString).toContain('client_id=test-client');
        expect(openedUrlString).toContain('code_challenge=');
        
        // Extract state from the opened URL
        const stateMatch = openedUrlString.match(/state=([^&]+)/);
        const state = stateMatch ? stateMatch[1] : null;
        expect(state).toBeTruthy();

        // 6. Simulate callback from browser via URI handler
        await authUriHandler.handleUri({
            toString: () => `vscode://rq-lang.rq-language/callback?code=auth-code-123&state=${state}`,
            query: `code=auth-code-123&state=${state}`
        } as any);

        // 7. Wait for command to complete
        await commandPromise;

        // 8. Verify Token Exchange
        expect(fetchMock.jestMock).toHaveBeenCalledWith(
            'https://auth.example.com/token',
            expect.objectContaining({
                method: 'POST',
                body: expect.stringContaining('code=auth-code-123')
            })
        );

        // 9. Verify Clipboard Copy
        expect(vscode.env.clipboard.writeText).toHaveBeenCalledWith('real-integration-token');
        expect(vscode.window.showInformationMessage).toHaveBeenCalledWith('Access token copied to clipboard');

        // 10. Verify Token Caching
        expect(context.secrets.store).toHaveBeenCalledWith(
            'oauth-token-cache', 
            expect.stringContaining('real-integration-token')
        );
    });

    test('uses cached token if available', async () => {
        // 1. Setup Cache with valid token
        const cachedToken = {
            accessToken: 'cached-token-123',
            expiresAt: Date.now() + 3600000, // 1 hour in future
            config: { clientId: 'test-client' }
        };
        // Cache key format: JSON.stringify({ authName: 'auth1', environment: 'dev' })
        const cacheKey = JSON.stringify({ authName: 'auth1', environment: 'dev' });
        mockSecrets.set('oauth-token-cache', JSON.stringify([[cacheKey, cachedToken]]));

        // 2. Setup CLI mocks
        (cliService.listEnvironments as jest.Mock).mockResolvedValue(['dev']);
        (cliService.listAuthConfigs as jest.Mock).mockResolvedValue([
            { name: 'auth1', auth_type: 'oauth2_authorization_code' }
        ]);
        (cliService.showAuthConfig as jest.Mock).mockResolvedValue({
            name: 'auth1',
            auth_type: 'oauth2_authorization_code',
            environment: 'dev',
            fields: {
                client_id: 'test-client',
                authorization_url: 'https://auth.example.com/authorize',
                token_url: 'https://auth.example.com/token',
                redirect_uri: 'vscode://rq-lang.rq-language/callback'
            }
        });

        // 3. Setup User Interactions
        mockEnvironmentPick('dev');
        (vscode.window.showQuickPick as jest.Mock)
            .mockResolvedValueOnce('auth1');
        
        (vscode.window.showInformationMessage as jest.Mock)
            .mockResolvedValueOnce('Copy Token');

        // 4. Execute command
        await commandCallback();

        // 5. Verify NO browser opening or fetch
        expect(vscode.env.openExternal).not.toHaveBeenCalled();
        expect(fetchMock.jestMock).not.toHaveBeenCalled();

        // 6. Verify Clipboard Copy of CACHED token
        expect(vscode.env.clipboard.writeText).toHaveBeenCalledWith('cached-token-123');
    });

    test('preselects the environment selected in the explorer', async () => {
        currentEnvironment = 'prod';
        (cliService.listEnvironments as jest.Mock).mockResolvedValue(['dev', 'prod']);
        const target = mockEnvironmentPick(undefined);

        await commandCallback();

        expect(target.activeItems.map(item => item.label)).toEqual(['prod']);
    });

    test('preselects no environment when none is selected in the explorer', async () => {
        (cliService.listEnvironments as jest.Mock).mockResolvedValue(['dev', 'prod']);
        const target = mockEnvironmentPick(undefined);

        await commandCallback();

        expect(target.activeItems.map(item => item.label)).toEqual(['(No environment)']);
    });

    test('stops when the environment pick is cancelled', async () => {
        (cliService.listEnvironments as jest.Mock).mockResolvedValue(['dev']);
        mockEnvironmentPick(undefined);

        await commandCallback();

        expect(cliService.listAuthConfigs).not.toHaveBeenCalled();
    });

    test('uses the environment accepted in the pick', async () => {
        currentEnvironment = 'prod';
        (cliService.listEnvironments as jest.Mock).mockResolvedValue(['dev', 'prod']);
        (cliService.listAuthConfigs as jest.Mock).mockResolvedValue([
            { name: 'auth1', auth_type: 'oauth2_authorization_code' }
        ]);
        (cliService.showAuthConfig as jest.Mock).mockRejectedValue(new Error('stop'));
        mockEnvironmentPick('dev');
        (vscode.window.showQuickPick as jest.Mock).mockResolvedValueOnce('auth1');

        await commandCallback();

        expect(cliService.showAuthConfig).toHaveBeenCalledWith('auth1', undefined, 'dev');
    });
});
