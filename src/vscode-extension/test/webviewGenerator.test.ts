import * as vscode from 'vscode';
import { getWebviewContent } from '../src/ui/webviewGenerator';
import * as cliService from '../src/cliService';

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
