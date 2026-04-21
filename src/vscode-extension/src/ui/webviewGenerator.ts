import * as vscode from 'vscode';
import * as rqClient from '../rqClient';

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

export function getErrorWebviewContent(
    requestName: string,
    errorMessage: string,
    requestDetails?: rqClient.RequestShowOutput
): string {
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

    return `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>RQ Request Error</title>
    <style>
        body {
            font-family: var(--vscode-font-family);
            font-size: var(--vscode-font-size);
            color: var(--vscode-foreground);
            background-color: var(--vscode-editor-background);
            padding: 20px;
            line-height: 1.6;
        }
        .header {
            border-bottom: 2px solid var(--vscode-panel-border);
            padding-bottom: 15px;
            margin-bottom: 20px;
        }
        .request-name {
            font-size: 1.5em;
            font-weight: bold;
            margin-bottom: 10px;
        }
        .request-info {
            display: flex;
            gap: 20px;
            margin-bottom: 10px;
        }
        .method {
            font-weight: bold;
            padding: 4px 8px;
            border-radius: 4px;
            background-color: var(--vscode-badge-background);
            color: var(--vscode-badge-foreground);
        }
        .url {
            font-family: var(--vscode-editor-font-family);
            color: var(--vscode-textLink-foreground);
        }
        .section {
            margin: 25px 0;
        }
        .section-title {
            font-weight: bold;
            font-size: 1.1em;
            margin-bottom: 10px;
            color: var(--vscode-foreground);
            cursor: pointer;
            user-select: none;
            display: flex;
            align-items: center;
            gap: 8px;
        }
        .section-title:hover { color: var(--vscode-textLink-foreground); }
        .section-title::before {
            content: '▼ ';
            display: inline-block;
            transition: transform 0.2s;
            margin-right: 6px;
        }
        .section-title.collapsed::before { transform: rotate(-90deg); }
        .section-content {
            overflow: hidden;
            transition: max-height 0.3s ease-out;
        }
        .section-content.collapsed { max-height: 0; }
        table { width: 100%; border-collapse: collapse; font-family: var(--vscode-editor-font-family); }
        td { padding: 6px 10px; border: 1px solid var(--vscode-panel-border); }
        .header-key { font-weight: bold; width: 30%; background-color: var(--vscode-editor-inactiveSelectionBackground); }
        .header-value { font-family: var(--vscode-editor-font-family); word-break: break-all; }
        .error-label {
            font-weight: bold;
            font-size: 1.1em;
            margin-bottom: 10px;
            color: #cf222e;
        }
        .error-message {
            background-color: var(--vscode-inputValidation-errorBackground, rgba(207, 34, 46, 0.1));
            border: 1px solid #cf222e;
            border-radius: 4px;
            padding: 15px;
            font-family: var(--vscode-editor-font-family);
            font-size: var(--vscode-editor-font-size);
            white-space: pre-wrap;
            word-break: break-word;
        }
        .run-again-btn {
            background-color: var(--vscode-button-background);
            color: var(--vscode-button-foreground);
            border: none;
            padding: 6px 12px;
            border-radius: 2px;
            cursor: pointer;
            font-family: var(--vscode-font-family);
            font-weight: bold;
            display: inline-flex;
            align-items: center;
            gap: 6px;
        }
        .run-again-btn:hover { background-color: var(--vscode-button-hoverBackground); }
    </style>
</head>
<body>
    <div class="header">
        <div style="display: flex; justify-content: space-between; align-items: center;">
            <div class="request-name">${escapeHtml(requestName)}</div>
            <button class="run-again-btn" onclick="runAgain()">
                <span>&#x21bb;</span> Run Again
            </button>
        </div>
        ${requestInfoHtml}
    </div>
    ${headersSection}
    <div class="section">
        <div class="error-label">Error</div>
        <div class="error-message">${escapeHtml(errorMessage)}</div>
    </div>
    <script>
        const vscode = acquireVsCodeApi();
        function runAgain() { vscode.postMessage({ command: 'runAgain' }); }
        function toggleSection(element) {
            element.classList.toggle('collapsed');
            element.nextElementSibling.classList.toggle('collapsed');
        }
    </script>
</body>
</html>`;
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