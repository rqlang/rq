import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import * as vscode from 'vscode';
import { getWebviewContent, getErrorWebviewContent, getViewPaneContent } from '../src/ui/webviewGenerator';
import * as cliService from '../src/rqClient';

describe('webviewGenerator', () => {
    // Mock context
    const mockContext = {
        extensionUri: { fsPath: '/mock/extension' }
    } as unknown as vscode.ExtensionContext;

    // Mock file system
    const mockFs = {
        readFile: jest.fn()
    };

    // Setup mocks
    beforeAll(() => {
        // Extend the existing vscode mock
        (vscode.workspace as any).fs = mockFs;
        (vscode.Uri as any).joinPath = jest.fn((base, ...segments) => ({ 
            fsPath: `${base.fsPath}/${segments.join('/')}` 
        }));
    });

    beforeEach(() => {
        jest.clearAllMocks();
    });

    test('getWebviewContent generates HTML with correct replacements', async () => {
        const mockTemplate = '<html><body>{{REQUEST_NAME}} {{METHOD}} {{URL}} {{STATUS}} {{BODY_CONTENT}} {{BODY_SCRIPT}}</body></html>';
        const mockRenderer = 'function render() {}';
        
        mockFs.readFile.mockImplementation((uri: any) => {
            if (uri.fsPath.endsWith('webviewTemplate.html')) {
                return Promise.resolve(new TextEncoder().encode(mockTemplate));
            }
            if (uri.fsPath.endsWith('jsonRenderer.js')) {
                return Promise.resolve(new TextEncoder().encode(mockRenderer));
            }
            return Promise.reject(new Error('File not found'));
        });

        const result: cliService.RequestExecutionResult = {
            request_name: 'Test Request',
            method: 'GET',
            url: 'https://api.example.com',
            status: 200,
            elapsed_ms: 100,
            request_headers: {},
            response_headers: {},
            body: '{"key": "value"}'
        };

        const html = await getWebviewContent(mockContext, result);

        expect(html).toContain('Test Request');
        expect(html).toContain('GET');
        expect(html).toContain('https://api.example.com');
        expect(html).toContain('200');

        expect(html).toContain('id="json-body"');
        expect(html).toContain('const jsonTree = JSON.parse("[\\"o\\",\\"key\\",\\"value\\"]");');
        expect(html).toContain(mockRenderer);
    });

    describe('getErrorWebviewContent', () => {
        const mockErrorTemplate = '<html><body>{{REQUEST_NAME}} {{REQUEST_INFO}} {{HEADERS_SECTION}} {{ERROR_MESSAGE}}</body></html>';

        beforeEach(() => {
            mockFs.readFile.mockImplementation((uri: any) => {
                if (uri.fsPath.endsWith('webviewErrorTemplate.html')) {
                    return Promise.resolve(new TextEncoder().encode(mockErrorTemplate));
                }
                return Promise.reject(new Error('File not found'));
            });
        });

        test('renders request name and error message', async () => {
            const html = await getErrorWebviewContent(mockContext, 'my-request', 'Connection refused');

            expect(html).toContain('my-request');
            expect(html).toContain('Connection refused');
        });

        test('renders method and url when request details provided', async () => {
            const details: cliService.RequestShowOutput = {
                name: 'my-request',
                method: 'POST',
                url: 'https://api.example.com/users',
                headers: {},
                file: 'test.rq',
                line: 1,
                character: 0,
                requiredVariables: []
            };

            const html = await getErrorWebviewContent(mockContext, 'my-request', 'Connection refused', details);

            expect(html).toContain('POST');
            expect(html).toContain('https://api.example.com/users');
        });

        test('renders request headers section when headers provided', async () => {
            const details: cliService.RequestShowOutput = {
                name: 'my-request',
                method: 'GET',
                url: 'http://localhost',
                headers: { 'Authorization': 'Bearer token', 'Accept': 'application/json' },
                file: 'test.rq',
                line: 1,
                character: 0,
                requiredVariables: []
            };

            const html = await getErrorWebviewContent(mockContext, 'my-request', 'error', details);

            expect(html).toContain('Request Headers (2)');
            expect(html).toContain('Authorization');
            expect(html).toContain('Bearer token');
            expect(html).toContain('Accept');
            expect(html).toContain('application/json');
        });

        test('renders empty request headers section when no headers defined', async () => {
            const details: cliService.RequestShowOutput = {
                name: 'my-request',
                method: 'GET',
                url: 'http://localhost',
                headers: {},
                file: 'test.rq',
                line: 1,
                character: 0,
                requiredVariables: []
            };

            const html = await getErrorWebviewContent(mockContext, 'my-request', 'error', details);

            expect(html).toContain('Request Headers (0)');
        });

        test('omits request details section when no details provided', async () => {
            const html = await getErrorWebviewContent(mockContext, 'my-request', 'error');

            expect(html).not.toContain('Request Headers');
            expect(html).not.toContain('class="method"');
        });

        test('keeps replacement patterns in the error message literal', async () => {
            const html = await getErrorWebviewContent(mockContext, 'my-request', 'bad $& value');

            expect(html).toContain('bad $&amp; value');
            expect(html).not.toContain('{{ERROR_MESSAGE}}');
        });

        test('escapes HTML special characters in error message', async () => {
            const html = await getErrorWebviewContent(mockContext, 'my-request', '<script>alert("xss")</script>');

            expect(html).toContain('&lt;script&gt;');
            expect(html).not.toContain('<script>alert');
        });
    });

    describe('copy buttons', () => {
        const baseResult: cliService.RequestExecutionResult = {
            request_name: 'Test',
            method: 'GET',
            url: 'https://api.example.com/users',
            status: 200,
            elapsed_ms: 50,
            request_headers: {},
            response_headers: {},
            body: 'plain text'
        };

        const renderWith = async (
            mockTemplate: string,
            overrides: Partial<cliService.RequestExecutionResult>
        ): Promise<string> => {
            mockFs.readFile.mockResolvedValue(new TextEncoder().encode(mockTemplate));
            return getWebviewContent(mockContext, { ...baseResult, ...overrides });
        };

        test('URL copy button receives the real URL in data-copy', async () => {
            const html = await renderWith(
                '<span class="url">{{URL}}</span><button data-copy="{{URL}}"></button>{{BODY_CONTENT}}{{BODY_SCRIPT}}',
                { url: 'https://api.example.com/users?id=1' }
            );

            expect(html).toContain('data-copy="https://api.example.com/users?id=1"');
            expect(html).not.toContain('{{URL}}');
        });

        test('each header row has a copy button with "name: value"', async () => {
            const html = await renderWith('{{REQUEST_HEADERS_HTML}}{{BODY_CONTENT}}{{BODY_SCRIPT}}', {
                request_headers: { 'Content-Type': 'application/json' }
            });

            expect(html).toContain('data-copy="Content-Type: application/json"');
        });

        test('copy-all headers button joins headers by newline', async () => {
            const html = await renderWith('{{RESPONSE_HEADERS_COPY_ALL}}{{BODY_CONTENT}}{{BODY_SCRIPT}}', {
                response_headers: { Accept: '*/*', Server: 'nginx' }
            });

            expect(html).toContain('class="copy-btn"');
            expect(html).toContain('data-copy="Accept: */*\nServer: nginx"');
            expect(html).not.toContain('{{RESPONSE_HEADERS_COPY_ALL}}');
        });

        test('request headers copy-all button renders for populated headers', async () => {
            const html = await renderWith('{{REQUEST_HEADERS_COPY_ALL}}{{BODY_CONTENT}}{{BODY_SCRIPT}}', {
                request_headers: { 'Content-Type': 'application/json' }
            });

            expect(html).toContain('class="copy-btn"');
            expect(html).toContain('data-copy="Content-Type: application/json"');
            expect(html).not.toContain('{{REQUEST_HEADERS_COPY_ALL}}');
        });

        test('copy-all headers button is omitted when there are no headers', async () => {
            const html = await renderWith('X{{REQUEST_HEADERS_COPY_ALL}}Y{{BODY_CONTENT}}{{BODY_SCRIPT}}', {
                request_headers: {}
            });

            expect(html).toContain('XY');
            expect(html).not.toContain('{{REQUEST_HEADERS_COPY_ALL}}');
        });

        test('keeps replacement patterns in a header value literal', async () => {
            const html = await renderWith('{{RESPONSE_HEADERS_HTML}}{{BODY_CONTENT}}', {
                response_headers: { 'X-Price': '5$&tax' }
            });

            expect(html).toContain('<td class="header-value">5$&amp;tax</td>');
        });

        test('leaves a placeholder inside a header value untouched', async () => {
            const html = await renderWith('{{RESPONSE_HEADERS_HTML}}<main>{{BODY_CONTENT}}</main>', {
                response_headers: { 'X-Debug': '{{BODY_CONTENT}}' }
            });

            expect(html).toContain('<td class="header-value">{{BODY_CONTENT}}</td>');
            expect(html).toContain('<main><pre class="text-body"><code>plain text</code></pre></main>');
        });

        test('escapes special characters in header copy data', async () => {
            const html = await renderWith('{{REQUEST_HEADERS_HTML}}{{BODY_CONTENT}}{{BODY_SCRIPT}}', {
                request_headers: { 'X-Test': '"><img src=x>' }
            });

            expect(html).toContain('X-Test: &quot;&gt;&lt;img src=x&gt;');
            expect(html).not.toContain('<img src=x>');
        });
    });

    test('result template blocks network loads and frame navigation', () => {
        const template = require('fs').readFileSync(require('path').join(__dirname, '../media/webviewTemplate.html'), 'utf8');

        expect(template).toContain(`<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; img-src data:; script-src 'unsafe-inline'; frame-src 'none'">`);
    });

    test('getWebviewContent handles non-JSON body', async () => {
        const mockTemplate = '{{BODY_CONTENT}} {{BODY_SCRIPT}}';
        mockFs.readFile.mockResolvedValue(new TextEncoder().encode(mockTemplate));

        const result: cliService.RequestExecutionResult = {
            request_name: 'Test',
            method: 'GET',
            url: 'http://test',
            status: 200,
            elapsed_ms: 50,
            request_headers: {},
            response_headers: {},
            body: 'Plain text body'
        };

        const html = await getWebviewContent(mockContext, result);

        expect(html).toContain('<pre class="text-body"><code>Plain text body</code></pre>');
        expect(html).not.toContain('const jsonTree =');
    });

    test('getWebviewContent renders a binary body instead of its text', async () => {
        mockFs.readFile.mockResolvedValue(new TextEncoder().encode('{{BODY_CONTENT}}'));

        const html = await getWebviewContent(mockContext, {
            request_name: 'Test',
            method: 'GET',
            url: 'http://test',
            status: 200,
            elapsed_ms: 50,
            request_headers: {},
            response_headers: { 'content-type': 'application/json' },
            body: '',
            body_file: { media_type: 'application/pdf', file_name: 'a.pdf', size: 8, binary: true, path: '/tmp/a.pdf' }
        });

        expect(html).toContain('Binary body (application/pdf, 8 bytes)');
        expect(html).not.toContain('json-body');
    });

    test('getWebviewContent says why the body was not written to its output', async () => {
        mockFs.readFile.mockResolvedValue(new TextEncoder().encode('{{BODY_CONTENT}}'));

        const html = await getWebviewContent(mockContext, {
            request_name: 'Test',
            method: 'GET',
            url: 'http://test',
            status: 404,
            elapsed_ms: 50,
            request_headers: {},
            response_headers: {},
            body: 'missing',
            output_skipped: { path: '/repo/<out>.json', reason: 'the response status is 404' }
        });

        expect(html).toContain('Body not written to <code>/repo/&lt;out&gt;.json</code>: the response status is 404');
    });

    test('getWebviewContent keeps replacement patterns in a text body literal', async () => {
        mockFs.readFile.mockResolvedValue(new TextEncoder().encode('{{BODY_CONTENT}}'));

        const result: cliService.RequestExecutionResult = {
            request_name: 'Test',
            method: 'GET',
            url: 'http://test',
            status: 200,
            elapsed_ms: 50,
            request_headers: {},
            response_headers: { 'content-type': 'text/plain' },
            body: "costs $& and $'"
        };

        const html = await getWebviewContent(mockContext, result);

        expect(html).toContain('costs $&amp; and $&#039;');
    });

    describe('response views', () => {
        let dir: string;

        beforeEach(() => {
            dir = fs.mkdtempSync(path.join(os.tmpdir(), 'rq-webview-view-'));
            mockFs.readFile.mockImplementation((uri: any) => {
                if (uri.fsPath.endsWith('webviewTemplate.html')) {
                    return Promise.resolve(new TextEncoder().encode('{{BODY_CONTENT}}<script>{{BODY_SCRIPT}}</script>'));
                }
                if (uri.fsPath.endsWith('pico.classless.min.css')) {
                    return Promise.resolve(new TextEncoder().encode('main{}'));
                }
                return Promise.resolve(new TextEncoder().encode(''));
            });
        });

        afterEach(() => {
            fs.rmSync(dir, { recursive: true, force: true });
        });

        function withView(template: string, status = 200): cliService.RequestExecutionResult {
            const file = path.join(dir, 'view.html');
            fs.writeFileSync(file, template);
            return {
                request_name: 'list',
                method: 'GET',
                url: 'http://localhost',
                status,
                elapsed_ms: 1,
                request_headers: {},
                response_headers: { 'content-type': 'application/json' },
                body: '{"id":7}',
                view: { path: file }
            };
        }

        test('shows the rendered view in a sandboxed frame before the body', async () => {
            const html = await getWebviewContent(mockContext, withView('<p>{{ body.id }}</p>'));

            expect(html.indexOf('<iframe class="response-view" title="Response view" sandbox=""')).toBeLessThan(html.indexOf('data-pane="body" hidden'));
        });

        test('keeps the body one tab away', async () => {
            const html = await getWebviewContent(mockContext, withView('<p>{{ body.id }}</p>'));

            expect(html).toContain('id="json-body"');
        });

        test('shows a template error with a button that opens the view at its line', async () => {
            const html = await getWebviewContent(mockContext, withView('<p></p>\n{% if %}'));

            expect(html).toContain('vscode.postMessage({&quot;command&quot;:&quot;openView&quot;,&quot;line&quot;:2})');
        });

        test('offers no button to open a view that does not exist', async () => {
            const result = { ...withView('<p></p>'), view: { path: path.join(dir, 'missing.html'), error: 'View missing.html not found' } };

            const html = await getWebviewContent(mockContext, result);

            expect([html.includes('View missing.html not found'), html.includes('openView')]).toEqual([true, false]);
        });

        test('gives the view tabs their own class so the HTML preview leaves them alone', async () => {
            const html = await getWebviewContent(mockContext, withView('<p></p>'));

            expect(html).toContain('<button class="body-tab active" data-pane="view"');
        });

        test('renders only the view pane for a refresh', async () => {
            const pane = await getViewPaneContent(mockContext, withView('<p>{{ body.id }}</p>'));

            expect([pane?.startsWith('<iframe class="response-view"'), pane?.includes('&lt;p&gt;7&lt;/p&gt;')]).toEqual([true, true]);
        });

        test('explains in the view pane why a refreshed view no longer applies', async () => {
            const pane = await getViewPaneContent(mockContext, withView('<p></p>', 404));

            expect(pane).toBe('<p class="output-skipped">View not applied: the response status is 404</p>');
        });

        test('renders the template open in the editor instead of the one on disk', async () => {
            const result = withView('<p>saved</p>');
            (vscode.workspace as any).textDocuments = [{ uri: { scheme: 'file', fsPath: result.view!.path }, getText: () => '<p>unsaved</p>' }];

            const pane = await getViewPaneContent(mockContext, result);
            (vscode.workspace as any).textDocuments = [];

            expect(pane).toContain('&lt;p&gt;unsaved&lt;/p&gt;');
        });

        test('renders the template open in the editor under the real path of a linked view', async () => {
            fs.mkdirSync(path.join(dir, 'real'));
            fs.writeFileSync(path.join(dir, 'real', 'orders.html'), '<p>saved</p>');
            fs.symlinkSync(path.join(dir, 'real'), path.join(dir, 'views'));
            const result = { ...withView('<p></p>'), view: { path: path.join(dir, 'views', 'orders.html') } };
            (vscode.workspace as any).textDocuments = [{ uri: { scheme: 'file', fsPath: path.join(dir, 'real', 'orders.html') }, getText: () => '<p>unsaved</p>' }];

            const pane = await getViewPaneContent(mockContext, result);
            (vscode.workspace as any).textDocuments = [];

            expect(pane).toContain('&lt;p&gt;unsaved&lt;/p&gt;');
        });

        test('keeps the frames of the panel from loading any page a view links to', () => {
            const target = fs.readFileSync(path.join(__dirname, '..', 'media', 'webviewTemplate.html'), 'utf8');

            expect(target).toMatch(/Content-Security-Policy" content="[^"]*frame-src 'none'/);
        });

        test('keeps the open view button out of the tabs', async () => {
            const html = await getWebviewContent(mockContext, withView('{% if %}'));

            expect(html).toContain('<button class="view-open" onclick=');
        });

        test('explains why a view was not applied', async () => {
            const html = await getWebviewContent(mockContext, withView('<p></p>', 500));

            expect(html).toContain('View not applied: the response status is 500');
        });
    });
});
