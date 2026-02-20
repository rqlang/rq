import * as vscode from 'vscode';
import * as cliService from '../cliService';

/**
 * Escape HTML special characters
 */
function escapeHtml(text: string): string {
    const map: Record<string, string> = {
        '&': '&amp;',
        '<': '&lt;',
        '>': '&gt;',
        '"': '&quot;',
        "'": '&#039;'
    };
    return text.replace(/[&<>"']/g, m => map[m]);
}

/**
 * Generate HTML content for the request result webview
 */
export async function getWebviewContent(context: vscode.ExtensionContext, result: cliService.RequestExecutionResult): Promise<string> {
    const statusClass = result.status >= 200 && result.status < 300 ? 'success' : 
                        result.status >= 400 ? 'error' : 'info';
    
    // Try to parse and pretty-print JSON body
    let formattedBody = result.body;
    let bodyLanguage = 'text';
    try {
        const parsed = JSON.parse(result.body);
        formattedBody = JSON.stringify(parsed, null, 2);
        bodyLanguage = 'json';
    } catch {
        // Not JSON, use as-is
    }
    
    // Format request headers as HTML table
    const requestHeadersHtml = Object.entries(result.request_headers)
        .map(([key, value]) => `<tr><td class="header-key">${escapeHtml(key)}</td><td class="header-value">${escapeHtml(value)}</td></tr>`)
        .join('\n');
    
    // Format response headers as HTML table
    const responseHeadersHtml = Object.entries(result.response_headers)
        .map(([key, value]) => `<tr><td class="header-key">${escapeHtml(key)}</td><td class="header-value">${escapeHtml(value)}</td></tr>`)
        .join('\n');
    
    // Read template file
    const templateUri = vscode.Uri.joinPath(context.extensionUri, 'media', 'webviewTemplate.html');
    const templateBuffer = await vscode.workspace.fs.readFile(templateUri);
    let html = new TextDecoder().decode(templateBuffer);

    // Read renderer script
    const rendererUri = vscode.Uri.joinPath(context.extensionUri, 'media', 'jsonRenderer.js');
    const rendererBuffer = await vscode.workspace.fs.readFile(rendererUri);
    const rendererScript = new TextDecoder().decode(rendererBuffer);

    // Prepare dynamic content
    const bodyContent = bodyLanguage === 'json' 
        ? `<div id="json-body"></div>` 
        : `<pre><code>${escapeHtml(formattedBody)}</code></pre>`;

    const jsonScript = bodyLanguage === 'json' ? `
        // JSON rendering with collapsible nodes
        const jsonData = ${formattedBody};
        
        ${rendererScript}
        
        renderJSON(jsonData, document.getElementById('json-body'));
    ` : '';

    // Replace placeholders
    html = html.replace('{{REQUEST_NAME}}', escapeHtml(result.request_name))
               .replace('{{METHOD}}', escapeHtml(result.method))
               .replace('{{URL}}', escapeHtml(result.url))
               .replace('{{STATUS_CLASS}}', statusClass)
               .replace('{{STATUS}}', result.status.toString())
               .replace('{{ELAPSED_MS}}', result.elapsed_ms.toString())
               .replace('{{REQUEST_HEADERS_COUNT}}', Object.keys(result.request_headers).length.toString())
               .replace('{{REQUEST_HEADERS_HTML}}', requestHeadersHtml)
               .replace('{{RESPONSE_HEADERS_COUNT}}', Object.keys(result.response_headers).length.toString())
               .replace('{{RESPONSE_HEADERS_HTML}}', responseHeadersHtml)
               .replace('{{BODY_CONTENT}}', bodyContent)
               .replace('{{JSON_SCRIPT}}', jsonScript);

    return html;
}