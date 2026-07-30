import * as vscode from 'vscode';
import * as rqClient from '../rqClient';

const COPY_ICON_SVG = '<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path></svg>';

function copyButton(value: string, title: string): string {
    return `<button class="copy-btn" onclick="copyValue(this)" data-copy="${escapeHtml(value)}" title="${escapeHtml(title)}">${COPY_ICON_SVG}</button>`;
}

function headerRowHtml(key: string, value: string): string {
    return `<tr><td class="header-key">${escapeHtml(key)}</td><td class="header-value">${escapeHtml(value)}</td><td class="header-copy">${copyButton(`${key}: ${value}`, 'Copy header to clipboard')}</td></tr>`;
}

function copyAllHeadersButton(headers: Record<string, string>): string {
    const entries = Object.entries(headers);
    if (entries.length === 0) {
        return '';
    }
    const allHeaders = entries.map(([key, value]) => `${key}: ${value}`).join('\n');
    return `<button class="copy-btn" onclick="event.stopPropagation(); copyValue(this)" data-copy="${escapeHtml(allHeaders)}" title="Copy all headers to clipboard">${COPY_ICON_SVG}</button>`;
}

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

export async function getErrorWebviewContent(
    context: vscode.ExtensionContext,
    requestName: string,
    errorMessage: string,
    requestDetails?: rqClient.RequestShowOutput
): Promise<string> {
    const requestInfoHtml = requestDetails ? `
        <div class="request-info">
            <span class="method">${escapeHtml(requestDetails.method)}</span>
            <span class="url">${escapeHtml(requestDetails.url)}</span>
        </div>` : '';

    const headers = requestDetails?.headers ?? {};
    const headersHtml = Object.entries(headers)
        .map(([k, v]) => `<tr><td class="header-key">${escapeHtml(k)}</td><td class="header-value">${escapeHtml(v)}</td></tr>`)
        .join('\n');

    const headersSection = requestDetails ? `
    <div class="section">
        <div class="section-title collapsed" onclick="toggleSection(this)">Request Headers (${Object.keys(headers).length})</div>
        <div class="section-content collapsed">
            <table>${headersHtml}</table>
        </div>
    </div>` : '';

    const templateUri = vscode.Uri.joinPath(context.extensionUri, 'media', 'webviewErrorTemplate.html');
    const templateBuffer = await vscode.workspace.fs.readFile(templateUri);
    const html = new TextDecoder().decode(templateBuffer);

    return html
        .replace('{{REQUEST_NAME}}', escapeHtml(requestName))
        .replace('{{REQUEST_INFO}}', requestInfoHtml)
        .replace('{{HEADERS_SECTION}}', headersSection)
        .replace('{{ERROR_MESSAGE}}', escapeHtml(errorMessage));
}

export async function getWebviewContent(context: vscode.ExtensionContext, result: rqClient.RequestExecutionResult): Promise<string> {
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
        .map(([key, value]) => headerRowHtml(key, value))
        .join('\n');

    // Format response headers as HTML table
    const responseHeadersHtml = Object.entries(result.response_headers)
        .map(([key, value]) => headerRowHtml(key, value))
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
               .replaceAll('{{URL}}', escapeHtml(result.url))
               .replace('{{STATUS_CLASS}}', statusClass)
               .replace('{{STATUS}}', result.status.toString())
               .replace('{{ELAPSED_MS}}', result.elapsed_ms.toString())
               .replace('{{REQUEST_HEADERS_COUNT}}', Object.keys(result.request_headers).length.toString())
               .replace('{{REQUEST_HEADERS_COPY_ALL}}', copyAllHeadersButton(result.request_headers))
               .replace('{{REQUEST_HEADERS_HTML}}', requestHeadersHtml)
               .replace('{{RESPONSE_HEADERS_COUNT}}', Object.keys(result.response_headers).length.toString())
               .replace('{{RESPONSE_HEADERS_COPY_ALL}}', copyAllHeadersButton(result.response_headers))
               .replace('{{RESPONSE_HEADERS_HTML}}', responseHeadersHtml)
               .replace('{{BODY_CONTENT}}', bodyContent)
               .replace('{{JSON_SCRIPT}}', jsonScript);

    return html;
}