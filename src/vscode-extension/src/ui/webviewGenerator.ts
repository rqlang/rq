import * as vscode from 'vscode';
import * as rqClient from '../rqClient';
import { bodyFileRenderer, getBodyRenderer, readMediaFile, RenderedBody } from './bodyRenderers';
import { escapeHtml, fillTemplate } from './html';
import { renderResponseView, ResponseView } from './responseView';

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

    return fillTemplate(html, {
        REQUEST_NAME: escapeHtml(requestName),
        REQUEST_INFO: requestInfoHtml,
        HEADERS_SECTION: headersSection,
        ERROR_MESSAGE: escapeHtml(errorMessage)
    });
}

export async function getWebviewContent(context: vscode.ExtensionContext, result: rqClient.RequestExecutionResult): Promise<string> {
    const statusClass = result.status >= 200 && result.status < 300 ? 'success' : 
                        result.status >= 400 ? 'error' : 'info';
    
    const bodyRenderer = result.body_file
        ? bodyFileRenderer(result.body_file)
        : getBodyRenderer(result.response_headers, result.body);
    const renderedBody = await withResponseView(context, result, await bodyRenderer.render(context, result.body));

    const requestHeadersHtml = Object.entries(result.request_headers)
        .map(([key, value]) => headerRowHtml(key, value))
        .join('\n');

    const responseHeadersHtml = Object.entries(result.response_headers)
        .map(([key, value]) => headerRowHtml(key, value))
        .join('\n');
    
    const templateUri = vscode.Uri.joinPath(context.extensionUri, 'media', 'webviewTemplate.html');
    const templateBuffer = await vscode.workspace.fs.readFile(templateUri);
    const html = new TextDecoder().decode(templateBuffer);

    return fillTemplate(html, {
        REQUEST_NAME: escapeHtml(result.request_name),
        METHOD: escapeHtml(result.method),
        URL: escapeHtml(result.url),
        STATUS_CLASS: statusClass,
        STATUS: result.status.toString(),
        ELAPSED_MS: result.elapsed_ms.toString(),
        REQUEST_HEADERS_COUNT: Object.keys(result.request_headers).length.toString(),
        REQUEST_HEADERS_COPY_ALL: copyAllHeadersButton(result.request_headers),
        REQUEST_HEADERS_HTML: requestHeadersHtml,
        RESPONSE_HEADERS_COUNT: Object.keys(result.response_headers).length.toString(),
        RESPONSE_HEADERS_COPY_ALL: copyAllHeadersButton(result.response_headers),
        RESPONSE_HEADERS_HTML: responseHeadersHtml,
        BODY_CONTENT: outputSkippedHtml(result.output_skipped) + renderedBody.content,
        BODY_SCRIPT: renderedBody.script
    });
}

export async function getViewPaneContent(context: vscode.ExtensionContext, result: rqClient.RequestExecutionResult): Promise<string | undefined> {
    const view = await renderView(context, result);
    return view === undefined || view.kind === 'skipped' ? undefined : responseViewHtml(view);
}

async function renderView(context: vscode.ExtensionContext, result: rqClient.RequestExecutionResult): Promise<ResponseView | undefined> {
    if (!result.view) {
        return undefined;
    }
    return renderResponseView(result.view, result, {
        trusted: vscode.workspace.isTrusted,
        theme: isDarkTheme(vscode.window.activeColorTheme.kind) ? 'dark' : 'light',
        stylesheet: await readViewStylesheet(context)
    });
}

async function withResponseView(context: vscode.ExtensionContext, result: rqClient.RequestExecutionResult, body: RenderedBody): Promise<RenderedBody> {
    const view = await renderView(context, result);
    if (view === undefined) {
        return body;
    }
    if (view.kind === 'skipped') {
        return { ...body, content: `<p class="output-skipped">View not applied: ${escapeHtml(view.reason)}</p>${body.content}` };
    }
    return {
        content: `<div class="body-tabs">
            <button class="body-tab active" data-pane="view" onclick="showBodyPane('view')">View</button>
            <button class="body-tab" data-pane="body" onclick="showBodyPane('body')">Body</button>
        </div>
        <div class="body-pane" data-pane="view">${responseViewHtml(view)}</div>
        <div class="body-pane" data-pane="body" hidden>${body.content}</div>`,
        script: `${body.script}
        function showBodyPane(pane) {
            document.querySelectorAll('.body-tab').forEach(button => button.classList.toggle('active', button.dataset.pane === pane));
            document.querySelectorAll('.body-pane').forEach(element => { element.hidden = element.dataset.pane !== pane; });
        }

        window.addEventListener('message', event => {
            if (event.data && event.data.command === 'updateView') {
                document.querySelector('.body-pane[data-pane="view"]').innerHTML = event.data.html;
            }
        });
    `
    };
}

function responseViewHtml(view: Exclude<ResponseView, { kind: 'skipped' }>): string {
    if (view.kind === 'rendered') {
        return `<iframe class="response-view" title="Response view" sandbox="" srcdoc="${escapeHtml(view.document)}"></iframe>`;
    }
    const message = escapeHtml(JSON.stringify({ command: 'openView', line: view.line }));
    const openButton = view.template === undefined
        ? ''
        : `<button class="body-tab active" onclick="vscode.postMessage(${message})">Open view</button>`;
    return `<div class="view-error">
            <p>${escapeHtml(view.message)}</p>
            ${openButton}
        </div>`;
}

function isDarkTheme(kind: vscode.ColorThemeKind): boolean {
    return kind === vscode.ColorThemeKind.Dark || kind === vscode.ColorThemeKind.HighContrast;
}

let viewStylesheet: Promise<string> | undefined;

function readViewStylesheet(context: vscode.ExtensionContext): Promise<string> {
    viewStylesheet ??= readMediaFile(context, 'pico.classless.min.css').catch((error: unknown) => {
        viewStylesheet = undefined;
        throw error;
    });
    return viewStylesheet;
}

function outputSkippedHtml(skipped?: rqClient.OutputSkipped): string {
    return skipped
        ? `<p class="output-skipped">Body not written to <code>${escapeHtml(skipped.path)}</code>: ${escapeHtml(skipped.reason)}</p>`
        : '';
}
