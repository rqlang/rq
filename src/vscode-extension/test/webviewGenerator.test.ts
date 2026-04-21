import * as vscode from 'vscode';
import { getWebviewContent, getErrorWebviewContent } from '../src/ui/webviewGenerator';
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
        // Mock template and renderer script
        const mockTemplate = '<html><body>{{REQUEST_NAME}} {{METHOD}} {{URL}} {{STATUS}} {{BODY_CONTENT}} {{JSON_SCRIPT}}</body></html>';
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
        
        // Check JSON specific content
        expect(html).toContain('id="json-body"'); // From {{BODY_CONTENT}} for JSON
        expect(html).toContain('const jsonData = {'); // From {{JSON_SCRIPT}}
        expect(html).toContain(mockRenderer); // Injected renderer script
    });

    describe('getErrorWebviewContent', () => {
        test('renders request name and error message', () => {
            const html = getErrorWebviewContent('my-request', 'Connection refused');

            expect(html).toContain('my-request');
            expect(html).toContain('Connection refused');
        });

        test('renders method and url when request details provided', () => {
            const details: cliService.RequestShowOutput = {
                name: 'my-request',
                method: 'POST',
                url: 'https://api.example.com/users',
                headers: {},
                file: 'test.rq',
                line: 1,
                character: 0
            };

            const html = getErrorWebviewContent('my-request', 'Connection refused', details);

            expect(html).toContain('POST');
            expect(html).toContain('https://api.example.com/users');
        });

        test('renders request headers section when headers provided', () => {
            const details: cliService.RequestShowOutput = {
                name: 'my-request',
                method: 'GET',
                url: 'http://localhost',
                headers: { 'Authorization': 'Bearer token', 'Accept': 'application/json' },
                file: 'test.rq',
                line: 1,
                character: 0
            };

            const html = getErrorWebviewContent('my-request', 'error', details);

            expect(html).toContain('Request Headers (2)');
            expect(html).toContain('Authorization');
            expect(html).toContain('Bearer token');
            expect(html).toContain('Accept');
            expect(html).toContain('application/json');
        });

        test('renders empty request headers section when no headers defined', () => {
            const details: cliService.RequestShowOutput = {
                name: 'my-request',
                method: 'GET',
                url: 'http://localhost',
                headers: {},
                file: 'test.rq',
                line: 1,
                character: 0
            };

            const html = getErrorWebviewContent('my-request', 'error', details);

            expect(html).toContain('Request Headers (0)');
        });

        test('omits request details section when no details provided', () => {
            const html = getErrorWebviewContent('my-request', 'error');

            expect(html).not.toContain('Request Headers');
            expect(html).not.toContain('class="method"');
        });

        test('escapes HTML special characters in error message', () => {
            const html = getErrorWebviewContent('my-request', '<script>alert("xss")</script>');

            expect(html).toContain('&lt;script&gt;');
            expect(html).not.toContain('<script>alert');
        });

        test('includes run again button', () => {
            const html = getErrorWebviewContent('my-request', 'error');

            expect(html).toContain('Run Again');
            expect(html).toContain('runAgain()');
        });
    });

    test('getWebviewContent handles non-JSON body', async () => {
        const mockTemplate = '{{BODY_CONTENT}} {{JSON_SCRIPT}}';
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

        expect(html).toContain('<pre><code>Plain text body</code></pre>');
        expect(html).not.toContain('const jsonData =');
    });
});
