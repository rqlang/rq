import * as vscode from 'vscode';
import { RequestRunner } from '../../src/commands/runRequest';
import { RequestExplorerProvider } from '../../src/requestExplorer';
import * as cliService from '../../src/rqClient';
import * as auth from '../../src/auth';
import * as webviewGenerator from '../../src/ui/webviewGenerator';

// Mock dependencies
jest.mock('../../src/requestExplorer', () => {
    const originalModule = jest.requireActual('../../src/requestExplorer');
    return {
        ...originalModule,
        RequestExplorerProvider: jest.fn().mockImplementation(() => ({
            getSelectedEnvironment: jest.fn(),
            setSelectedEnvironment: jest.fn(),
            refresh: jest.fn(),
            getTreeItem: jest.fn(),
            getChildren: jest.fn(),
            setItemLoading: jest.fn(),
            setTreeLoading: jest.fn()
        }))
    };
});
jest.mock('../../src/rqClient');
jest.mock('../../src/auth');
jest.mock('../../src/ui/webviewGenerator');

// Import RequestTreeItem after mocking
import { RequestTreeItem } from '../../src/requestExplorer';

describe('runRequest Commands', () => {
    let context: vscode.ExtensionContext;
    let provider: RequestExplorerProvider;
    let outputChannel: vscode.OutputChannel;
    let requestRunner: RequestRunner;
    let mockWebviewPanel: any;

    beforeEach(() => {
        jest.clearAllMocks();

        // Mock context
        context = {
            subscriptions: []
        } as unknown as vscode.ExtensionContext;

        // Mock provider
        provider = new RequestExplorerProvider('root');
        provider.getSelectedEnvironment = jest.fn().mockReturnValue('dev');

        // Mock output channel
        outputChannel = {
            appendLine: jest.fn(),
            show: jest.fn()
        } as unknown as vscode.OutputChannel;

        // Mock Webview Panel
        mockWebviewPanel = {
            webview: { 
                html: '',
                onDidReceiveMessage: jest.fn()
            },
            reveal: jest.fn(),
            onDidDispose: jest.fn(),
            title: ''
        };
        (vscode.window.createWebviewPanel as jest.Mock).mockImplementation(() => mockWebviewPanel);

        // Mock withProgress to execute callback immediately
        (vscode.window.withProgress as jest.Mock).mockImplementation(async (options, callback) => {
            return callback();
        });

        // Instantiate RequestRunner
        requestRunner = new RequestRunner(context, outputChannel);

        // Mock cliService defaults
        (cliService as any).showRequest.mockResolvedValue({
            name: 'test-req',
            method: 'GET',
            url: 'http://localhost',
            headers: {},
            file: 'test.rq',
            line: 1,
            character: 0,
            requiredVariables: []
        });
        (cliService as any).executeRequest.mockResolvedValue({
            results: [{
                request_name: 'test-req',
                status: 200,
                body: '{}',
                elapsed_ms: 100,
                method: 'GET',
                url: 'http://localhost',
                response_headers: { 'content-type': 'application/json' },
                request_headers: {}
            }],
            stderr: ''
        });

        // Mock auth
        (auth as any).performOAuth2Flow.mockResolvedValue('mock-token');

        // Mock webview
        (webviewGenerator as any).getWebviewContent.mockReturnValue('<html></html>');
        (webviewGenerator as any).getErrorWebviewContent.mockResolvedValue('<html>error</html>');
    });

    describe('rq.runRequest', () => {
        test('runs request successfully', async () => {
            const request = { name: 'test-req', endpoint: 'GET /', file: 'test.rq' };
            const item = new RequestTreeItem('test-req', request, 0);

            (cliService.showRequest as jest.Mock).mockResolvedValue({ name: 'test-req', requiredVariables: [] });
            (cliService.executeRequest as jest.Mock).mockResolvedValue({
                results: [{ 
                    status: 200, 
                    body: '{}',
                    elapsed_ms: 100,
                    method: 'GET',
                    url: 'http://localhost',
                    response_headers: { 'content-type': 'application/json' },
                    request_headers: {}
                }],
                stderr: ''
            });
            (webviewGenerator.getWebviewContent as jest.Mock).mockResolvedValue('<html></html>');

            await requestRunner.runRequest(item, provider);

            expect(cliService.executeRequest).toHaveBeenCalledWith(expect.objectContaining({
                requestName: 'test-req',
                environment: 'dev'
            }));
            expect(vscode.window.createWebviewPanel).toHaveBeenCalled();
            expect(mockWebviewPanel.webview.html).toBe('<html></html>');
            expect(provider.setItemLoading).toHaveBeenCalledTimes(2);
            expect(provider.setItemLoading).toHaveBeenNthCalledWith(1, item, true);
            expect(provider.setItemLoading).toHaveBeenNthCalledWith(2, item, false);
        });

        test('handles OAuth2 auth', async () => {
            const request = { name: 'auth-req', endpoint: 'GET /', file: 'test.rq' };
            const item = new RequestTreeItem('auth-req', request, 0);

            (cliService.showRequest as jest.Mock).mockResolvedValue({
                name: 'auth-req',
                auth: { name: 'my-auth', type: 'oauth2_authorization_code' },
                requiredVariables: []
            });
            (cliService.showAuthConfig as jest.Mock).mockResolvedValue({ name: 'my-auth' });
            (auth.performOAuth2Flow as jest.Mock).mockResolvedValue('mock-token');
            (cliService.executeRequest as jest.Mock).mockResolvedValue({ results: [{}] });

            await requestRunner.runRequest(item, provider);

            expect(auth.performOAuth2Flow).toHaveBeenCalled();
            expect(cliService.executeRequest).toHaveBeenCalledWith(expect.objectContaining({
                variables: { auth_token: 'mock-token' }
            }));
            expect(provider.setItemLoading).toHaveBeenCalledTimes(2);
            expect(provider.setItemLoading).toHaveBeenNthCalledWith(1, item, true);
            expect(provider.setItemLoading).toHaveBeenNthCalledWith(2, item, false);
        });

        test('handles OAuth2 auth with required variables', async () => {
            const request = { name: 'auth-req', endpoint: 'GET /', file: 'test.rq' };
            const item = new RequestTreeItem('auth-req', request, 0);

            (cliService.showRequest as jest.Mock).mockResolvedValue({
                name: 'auth-req',
                auth: { name: 'my-auth', type: 'oauth2_authorization_code' },
                requiredVariables: ['user_id']
            });
            (cliService.showAuthConfig as jest.Mock).mockResolvedValue({ name: 'my-auth' });
            (auth.performOAuth2Flow as jest.Mock).mockResolvedValue('mock-token');
            (vscode.window.showInputBox as jest.Mock).mockResolvedValueOnce('42');
            (cliService.executeRequest as jest.Mock).mockResolvedValue({ results: [{}] });

            await requestRunner.runRequest(item, provider);

            expect(auth.performOAuth2Flow).toHaveBeenCalled();
            expect(cliService.executeRequest).toHaveBeenCalledWith(expect.objectContaining({
                variables: { auth_token: 'mock-token', user_id: '42' }
            }));
        });

        test('handles execution error', async () => {
            const request = { name: 'error-req', endpoint: 'GET /', file: 'test.rq' };
            const item = new RequestTreeItem('error-req', request, 0);

            (cliService.executeRequest as jest.Mock).mockRejectedValue(new Error('Execution failed'));

            await requestRunner.runRequest(item, provider);

            expect(vscode.window.showErrorMessage).not.toHaveBeenCalled();
            expect(vscode.window.createWebviewPanel).toHaveBeenCalled();
            expect(webviewGenerator.getErrorWebviewContent).toHaveBeenCalledWith(
                expect.anything(),
                'error-req',
                'Execution failed',
                expect.anything()
            );
            expect(mockWebviewPanel.webview.html).toBe('<html>error</html>');
            expect(provider.setItemLoading).toHaveBeenCalledTimes(2);
            expect(provider.setItemLoading).toHaveBeenNthCalledWith(1, item, true);
            expect(provider.setItemLoading).toHaveBeenNthCalledWith(2, item, false);
        });

        test('filters cargo output from error message', async () => {
            const request = { name: 'fail-req', endpoint: 'GET /', file: 'test.rq' };
            const item = new RequestTreeItem('fail-req', request, 0);

            const stderr = [
                'Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.07s',
                'Running `target/debug/rq request run -n api/get`',
                'error: invalid value',
                'For more information, try \'--help\'.'
            ].join('\n');

            (cliService.executeRequest as jest.Mock).mockResolvedValue({
                results: [],
                stderr: stderr
            });

            await requestRunner.runRequest(item, provider);

            expect(vscode.window.showErrorMessage).not.toHaveBeenCalled();
            expect(vscode.window.createWebviewPanel).toHaveBeenCalled();
            expect(webviewGenerator.getErrorWebviewContent).toHaveBeenCalledWith(
                expect.anything(),
                'fail-req',
                'error: invalid value',
                expect.anything()
            );
            expect(mockWebviewPanel.webview.html).toBe('<html>error</html>');
            expect(provider.setItemLoading).toHaveBeenCalledTimes(2);
            expect(provider.setItemLoading).toHaveBeenNthCalledWith(1, item, true);
            expect(provider.setItemLoading).toHaveBeenNthCalledWith(2, item, false);
        });
    });

    describe('rq.runRequestWithVariables', () => {
        test('prompts for variables and runs request', async () => {
            const request = { name: 'var-req', endpoint: 'GET /', file: 'test.rq' };
            const item = new RequestTreeItem('var-req', request, 0);

            (cliService.showRequest as jest.Mock).mockResolvedValue({ name: 'var-req', requiredVariables: [] });

            // Mock user input: var1=val1, var2=val2, then empty string to stop
            (vscode.window.showInputBox as jest.Mock)
                .mockResolvedValueOnce('var1=val1')
                .mockResolvedValueOnce('var2=val2')
                .mockResolvedValueOnce('');

            (cliService.executeRequest as jest.Mock).mockResolvedValue({ results: [{}] });

            await requestRunner.runRequestWithVariables(item, provider);

            expect(cliService.executeRequest).toHaveBeenCalledWith(expect.objectContaining({
                variables: { var1: 'val1', var2: 'val2' }
            }));
            expect(provider.setItemLoading).toHaveBeenCalledTimes(2);
            expect(provider.setItemLoading).toHaveBeenNthCalledWith(1, item, true);
            expect(provider.setItemLoading).toHaveBeenNthCalledWith(2, item, false);
        });

        test('cancels if input cancelled', async () => {
            const request = { name: 'cancel-req', endpoint: 'GET /', file: 'test.rq' };
            const item = new RequestTreeItem('cancel-req', request, 0);

            (cliService.showRequest as jest.Mock).mockResolvedValue({ name: 'cancel-req', requiredVariables: [] });
            (vscode.window.showInputBox as jest.Mock).mockResolvedValue(undefined);

            await requestRunner.runRequestWithVariables(item, provider);

            expect(cliService.executeRequest).not.toHaveBeenCalled();
            expect(provider.setItemLoading).toHaveBeenCalledTimes(2);
            expect(provider.setItemLoading).toHaveBeenNthCalledWith(1, item, true);
            expect(provider.setItemLoading).toHaveBeenNthCalledWith(2, item, false);
        });

        test('prompts for required variables not yet supplied', async () => {
            const request = { name: 'req-req', endpoint: 'GET /', file: 'test.rq' };
            const item = new RequestTreeItem('req-req', request, 0);

            (cliService.showRequest as jest.Mock).mockResolvedValue({ name: 'req-req', requiredVariables: ['user_id'] });

            // user input flow: empty string stops collectUserVariables, then required prompt supplies user_id
            (vscode.window.showInputBox as jest.Mock)
                .mockResolvedValueOnce('')       // collectUserVariables: stop immediately
                .mockResolvedValueOnce('42');    // collectRequiredVariables: user_id

            (cliService.executeRequest as jest.Mock).mockResolvedValue({ results: [{}] });

            await requestRunner.runRequestWithVariables(item, provider);

            expect(cliService.executeRequest).toHaveBeenCalledWith(expect.objectContaining({
                variables: { user_id: '42' }
            }));
        });

        test('clears loading state when execution throws', async () => {
            const request = { name: 'throw-req', endpoint: 'GET /', file: 'test.rq' };
            const item = new RequestTreeItem('throw-req', request, 0);

            (cliService.showRequest as jest.Mock).mockResolvedValue({ name: 'throw-req', requiredVariables: [] });
            (vscode.window.showInputBox as jest.Mock).mockResolvedValue('');
            (cliService.executeRequest as jest.Mock).mockRejectedValue(new Error('CLI crashed'));

            await requestRunner.runRequestWithVariables(item, provider);

            expect(vscode.window.showErrorMessage).not.toHaveBeenCalled();
            expect(vscode.window.createWebviewPanel).toHaveBeenCalled();
            expect(webviewGenerator.getErrorWebviewContent).toHaveBeenCalledWith(
                expect.anything(),
                'throw-req',
                'CLI crashed',
                expect.anything()
            );
            expect(provider.setItemLoading).toHaveBeenCalledTimes(2);
            expect(provider.setItemLoading).toHaveBeenNthCalledWith(1, item, true);
            expect(provider.setItemLoading).toHaveBeenNthCalledWith(2, item, false);
        });
    });
});
