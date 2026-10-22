import * as vscode from 'vscode';
import { RequestRunner } from '../../src/commands/runRequest';
import { RequestExplorerProvider } from '../../src/requestExplorer';
import { createExplorerState } from '../memento-mock';
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
            setItemRunning: jest.fn(),
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
        provider = new RequestExplorerProvider('root', createExplorerState());
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
                onDidReceiveMessage: jest.fn(),
                postMessage: jest.fn()
            },
            reveal: jest.fn(),
            onDidDispose: jest.fn(),
            title: ''
        };
        (vscode.window.createWebviewPanel as jest.Mock).mockImplementation(() => mockWebviewPanel);

        // Mock withProgress to execute callback immediately
        (vscode.window.withProgress as jest.Mock).mockImplementation(async (options, callback) => {
            return callback(
                { report: jest.fn() },
                { isCancellationRequested: false, onCancellationRequested: jest.fn().mockReturnValue({ dispose: jest.fn() }) }
            );
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
            expect(provider.setItemRunning).toHaveBeenCalledTimes(2);
            expect(provider.setItemRunning).toHaveBeenNthCalledWith(1, item, true);
            expect(provider.setItemRunning).toHaveBeenNthCalledWith(2, item, false);
        });

        test('acquires an OAuth2 token when rq-lib asks for one', async () => {
            const request = { name: 'auth-req', endpoint: 'GET /', file: 'test.rq' };
            const item = new RequestTreeItem('auth-req', request, 0);
            const pendingAuth = {
                name: 'my-auth',
                auth_type: 'oauth2_authorization_code',
                fields: {},
                file: 'test.rq',
                line: 0,
                character: 0,
                token_required: true,
            };

            (cliService.showRequest as jest.Mock).mockResolvedValue({ name: 'auth-req', requiredVariables: [] });
            (auth.performOAuth2Flow as jest.Mock).mockResolvedValue('mock-token');
            let acquired: string | undefined;
            (cliService.executeRequest as jest.Mock).mockImplementation(async (options: cliService.ExecuteRequestOptions) => {
                acquired = await options.acquireToken?.(pendingAuth);
                return { results: [{}] };
            });

            await requestRunner.runRequest(item, provider);

            expect(auth.performOAuth2Flow).toHaveBeenCalledWith(pendingAuth, context, outputChannel);
            expect(acquired).toBe('mock-token');
            expect(provider.setItemRunning).toHaveBeenCalledTimes(2);
            expect(provider.setItemRunning).toHaveBeenNthCalledWith(1, item, true);
            expect(provider.setItemRunning).toHaveBeenNthCalledWith(2, item, false);
        });

        test('runs the request against the file that declares it with its required variables', async () => {
            const request = { name: 'auth-req', endpoint: 'GET /', file: '/workspace/sub/test.rq' };
            const item = new RequestTreeItem('auth-req', request, 0);

            (cliService.showRequest as jest.Mock).mockResolvedValue({ name: 'auth-req', requiredVariables: ['user_id'] });
            (vscode.window.showInputBox as jest.Mock).mockResolvedValueOnce('42');
            (cliService.executeRequest as jest.Mock).mockResolvedValue({ results: [{}] });

            await requestRunner.runRequest(item, provider);

            expect(cliService.executeRequest).toHaveBeenCalledWith(expect.objectContaining({
                sourceFile: '/workspace/sub/test.rq',
                variables: { user_id: '42' }
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
            expect(provider.setItemRunning).toHaveBeenCalledTimes(2);
            expect(provider.setItemRunning).toHaveBeenNthCalledWith(1, item, true);
            expect(provider.setItemRunning).toHaveBeenNthCalledWith(2, item, false);
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
            expect(provider.setItemRunning).toHaveBeenCalledTimes(2);
            expect(provider.setItemRunning).toHaveBeenNthCalledWith(1, item, true);
            expect(provider.setItemRunning).toHaveBeenNthCalledWith(2, item, false);
        });
    });

    describe('webview messages', () => {
        const runAndGetMessageHandler = async (): Promise<(message: any) => void> => {
            const request = { name: 'test-req', endpoint: 'GET /', file: 'test.rq' };
            const item = new RequestTreeItem('test-req', request, 0);

            (cliService.executeRequest as jest.Mock).mockResolvedValue({
                results: [{
                    request_name: 'test-req',
                    status: 200,
                    body: '{"key":"value"}',
                    elapsed_ms: 100,
                    method: 'GET',
                    url: 'http://localhost',
                    response_headers: {},
                    request_headers: {}
                }],
                stderr: ''
            });

            await requestRunner.runRequest(item, provider);

            return mockWebviewPanel.webview.onDidReceiveMessage.mock.calls[0][0];
        };

        test('copy command writes provided text to the clipboard', async () => {
            const handler = await runAndGetMessageHandler();

            handler({ command: 'copy', text: 'Content-Type: application/json' });

            expect(vscode.env.clipboard.writeText).toHaveBeenCalledWith('Content-Type: application/json');
            expect(vscode.window.showInformationMessage).toHaveBeenCalledWith('Copied to clipboard');
        });

        test('copy command ignores messages without string text', async () => {
            const handler = await runAndGetMessageHandler();

            handler({ command: 'copy' });

            expect(vscode.env.clipboard.writeText).not.toHaveBeenCalled();
        });

        test('copyBody command writes the last response body to the clipboard', async () => {
            const handler = await runAndGetMessageHandler();

            handler({ command: 'copyBody' });

            expect(vscode.env.clipboard.writeText).toHaveBeenCalledWith('{"key":"value"}');
            expect(vscode.window.showInformationMessage).toHaveBeenCalledWith('Body copied to clipboard');
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
            expect(provider.setItemRunning).toHaveBeenCalledTimes(2);
            expect(provider.setItemRunning).toHaveBeenNthCalledWith(1, item, true);
            expect(provider.setItemRunning).toHaveBeenNthCalledWith(2, item, false);
        });

        test('cancels if input cancelled', async () => {
            const request = { name: 'cancel-req', endpoint: 'GET /', file: 'test.rq' };
            const item = new RequestTreeItem('cancel-req', request, 0);

            (cliService.showRequest as jest.Mock).mockResolvedValue({ name: 'cancel-req', requiredVariables: [] });
            (vscode.window.showInputBox as jest.Mock).mockResolvedValue(undefined);

            await requestRunner.runRequestWithVariables(item, provider);

            expect(cliService.executeRequest).not.toHaveBeenCalled();
            expect(provider.setItemRunning).toHaveBeenCalledTimes(2);
            expect(provider.setItemRunning).toHaveBeenNthCalledWith(1, item, true);
            expect(provider.setItemRunning).toHaveBeenNthCalledWith(2, item, false);
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
            expect(provider.setItemRunning).toHaveBeenCalledTimes(2);
            expect(provider.setItemRunning).toHaveBeenNthCalledWith(1, item, true);
            expect(provider.setItemRunning).toHaveBeenNthCalledWith(2, item, false);
        });
    });
    describe('cancellation', () => {
        function makeItem(name: string) {
            return new RequestTreeItem(name, { name, endpoint: 'GET /', file: 'test.rq' }, 0);
        }

        test('passes a cancellation token to executeRequest', async () => {
            (cliService.showRequest as jest.Mock).mockResolvedValue({ name: 'test-req', requiredVariables: [] });

            await requestRunner.runRequest(makeItem('test-req'), provider);

            expect(cliService.executeRequest).toHaveBeenCalledWith(expect.objectContaining({
                cancellation: expect.objectContaining({ isCancellationRequested: false })
            }));
        });

        test('sets and clears the running context key', async () => {
            (cliService.showRequest as jest.Mock).mockResolvedValue({ name: 'test-req', requiredVariables: [] });

            await requestRunner.runRequest(makeItem('test-req'), provider);

            expect(vscode.commands.executeCommand).toHaveBeenCalledWith('setContext', 'rq.requestRunning', true);
            expect(vscode.commands.executeCommand).toHaveBeenCalledWith('setContext', 'rq.requestRunning', false);
        });

        test('refuses to start a second request while one is running', async () => {
            (cliService.showRequest as jest.Mock).mockResolvedValue({ name: 'test-req', requiredVariables: [] });
            (cliService.executeRequest as jest.Mock).mockImplementation(() => new Promise(() => {}));

            const first = requestRunner.runRequest(makeItem('test-req'), provider);
            await new Promise(resolve => setImmediate(resolve));
            await requestRunner.runRequest(makeItem('other-req'), provider);

            expect(cliService.executeRequest).toHaveBeenCalledTimes(1);
            expect(vscode.window.showWarningMessage).toHaveBeenCalledWith(
                expect.stringContaining('already running')
            );
            expect(requestRunner.isExecuting()).toBe(true);
            void first;
        });

        test('cancelActiveExecution marks the token passed to executeRequest', async () => {
            (cliService.showRequest as jest.Mock).mockResolvedValue({ name: 'test-req', requiredVariables: [] });
            (cliService.executeRequest as jest.Mock).mockImplementation(() => new Promise(() => {}));

            const pending = requestRunner.runRequest(makeItem('test-req'), provider);
            await new Promise(resolve => setImmediate(resolve));
            requestRunner.cancelActiveExecution();

            const { cancellation } = (cliService.executeRequest as jest.Mock).mock.calls[0][0];
            expect(cancellation.isCancellationRequested).toBe(true);
            void pending;
        });

        test('does not open an error panel when the run is cancelled', async () => {
            (cliService.showRequest as jest.Mock).mockResolvedValue({ name: 'test-req', requiredVariables: [] });
            (cliService.executeRequest as jest.Mock).mockRejectedValue(new Error('Cancelled by user'));

            await requestRunner.runRequest(makeItem('test-req'), provider);

            expect(webviewGenerator.getErrorWebviewContent).not.toHaveBeenCalled();
            expect(requestRunner.isExecuting()).toBe(false);
        });

        test('clears the running state on the tree item after cancelling', async () => {
            const item = makeItem('test-req');
            (cliService.showRequest as jest.Mock).mockResolvedValue({ name: 'test-req', requiredVariables: [] });
            (cliService.executeRequest as jest.Mock).mockRejectedValue(new Error('Cancelled by user'));

            await requestRunner.runRequest(item, provider);

            expect(provider.setItemRunning).toHaveBeenNthCalledWith(2, item, false);
        });
    });

    describe('results tabs', () => {
        let panels: any[];

        function makeItem(name: string) {
            return new RequestTreeItem(name, { name, endpoint: 'GET /', file: 'test.rq' }, 0);
        }

        function useTabMode(mode: string) {
            (vscode.workspace.getConfiguration as jest.Mock).mockReturnValue({
                get: jest.fn().mockReturnValue(mode)
            });
        }

        function respondWith(body: string) {
            (cliService.executeRequest as jest.Mock).mockResolvedValueOnce({
                results: [{
                    status: 200,
                    body,
                    elapsed_ms: 1,
                    method: 'GET',
                    url: 'http://localhost',
                    response_headers: {},
                    request_headers: {}
                }],
                stderr: ''
            });
        }

        function messageHandler(panel: any): (message: any) => void {
            return panel.webview.onDidReceiveMessage.mock.calls[0][0];
        }

        async function settle() {
            for (let i = 0; i < 10; i++) {
                await new Promise(resolve => setImmediate(resolve));
            }
        }

        beforeEach(() => {
            panels = [];
            (vscode.window.createWebviewPanel as jest.Mock).mockImplementation((_viewType: string, title: string) => {
                const panel = {
                    webview: { html: '', onDidReceiveMessage: jest.fn(), postMessage: jest.fn() },
                    reveal: jest.fn(),
                    onDidDispose: jest.fn(),
                    title
                };
                panels.push(panel);
                return panel;
            });
            (cliService.showRequest as jest.Mock).mockResolvedValue({ name: 'req', requiredVariables: [] });
        });

        afterEach(() => {
            (vscode.workspace.getConfiguration as jest.Mock).mockReturnValue({
                get: jest.fn().mockReturnValue(false)
            });
        });

        test('single mode shows every request in one tab', async () => {
            useTabMode('single');
            respondWith('{"a":1}');
            respondWith('{"b":1}');

            await requestRunner.runRequest(makeItem('a'), provider);
            await requestRunner.runRequest(makeItem('b'), provider);

            expect(panels).toHaveLength(1);
            expect(panels[0].title).toBe('RQ: b');
        });

        test('perRequest mode opens one tab per request', async () => {
            useTabMode('perRequest');
            respondWith('{"a":1}');
            respondWith('{"b":1}');
            respondWith('{"a":2}');

            await requestRunner.runRequest(makeItem('a'), provider);
            await requestRunner.runRequest(makeItem('b'), provider);
            await requestRunner.runRequest(makeItem('a'), provider);

            expect(panels.map(panel => panel.title)).toEqual(['RQ: a', 'RQ: b']);
        });

        test('perExecution mode opens a tab per run', async () => {
            useTabMode('perExecution');
            respondWith('{"a":1}');
            respondWith('{"a":2}');

            await requestRunner.runRequest(makeItem('a'), provider);
            await requestRunner.runRequest(makeItem('a'), provider);

            expect(panels).toHaveLength(2);
        });

        test('run again repeats the request of the tab it is clicked in', async () => {
            useTabMode('perRequest');
            respondWith('{"a":1}');
            respondWith('{"b":1}');
            respondWith('{"a":2}');
            await requestRunner.runRequest(makeItem('a'), provider);
            await requestRunner.runRequest(makeItem('b'), provider);

            messageHandler(panels[0])({ command: 'runAgain' });
            await settle();

            expect(cliService.executeRequest).toHaveBeenLastCalledWith(expect.objectContaining({ requestName: 'a' }));
        });

        test('run again in perExecution mode refreshes its own tab', async () => {
            useTabMode('perExecution');
            respondWith('{"a":1}');
            respondWith('{"a":2}');
            await requestRunner.runRequest(makeItem('a'), provider);

            messageHandler(panels[0])({ command: 'runAgain' });
            await settle();

            expect(cliService.executeRequest).toHaveBeenCalledTimes(2);
            expect(panels).toHaveLength(1);
        });

        test('copy body copies the body of the tab it is clicked in', async () => {
            useTabMode('perRequest');
            respondWith('{"a":1}');
            respondWith('{"b":1}');
            await requestRunner.runRequest(makeItem('a'), provider);
            await requestRunner.runRequest(makeItem('b'), provider);

            messageHandler(panels[0])({ command: 'copyBody' });

            expect(vscode.env.clipboard.writeText).toHaveBeenCalledWith('{"a":1}');
        });

        test('execution state is posted only to the running tab', async () => {
            useTabMode('perRequest');
            respondWith('{"a":1}');
            respondWith('{"b":1}');
            respondWith('{"a":2}');
            await requestRunner.runRequest(makeItem('a'), provider);
            await requestRunner.runRequest(makeItem('b'), provider);
            panels.forEach(panel => panel.webview.postMessage.mockClear());

            await requestRunner.runRequest(makeItem('a'), provider);

            expect(panels[0].webview.postMessage).toHaveBeenCalledWith({ command: 'executionState', running: true });
            expect(panels[1].webview.postMessage).not.toHaveBeenCalled();
        });

        test('an error is shown in the tab of the failing request', async () => {
            useTabMode('perRequest');
            respondWith('{"a":1}');
            (cliService.executeRequest as jest.Mock).mockRejectedValueOnce(new Error('boom'));
            await requestRunner.runRequest(makeItem('a'), provider);

            await requestRunner.runRequest(makeItem('b'), provider);

            expect(panels).toHaveLength(2);
            expect(panels[1].title).toBe('RQ: b');
            expect(panels[1].webview.html).toBe('<html>error</html>');
        });
    });
});
