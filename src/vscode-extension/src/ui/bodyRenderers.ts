import * as vscode from 'vscode';
import type { BinaryBody } from '../execution';
import { escapeHtml } from './html';
import { parseJsonTree, serializeJsonTree } from './jsonTree';

export interface RenderedBody {
    content: string;
    script: string;
}

export interface BodyRenderer {
    render(context: vscode.ExtensionContext, body: string): Promise<RenderedBody>;
}

type MediaTypeRenderer = (mediaType: string | undefined, body: string) => BodyRenderer | undefined;

function jsonRenderer(mediaType: string | undefined, body: string): BodyRenderer | undefined {
    const tree = mediaType === undefined || isJsonMediaType(mediaType) ? parseJsonTree(body) : undefined;
    if (tree === undefined) {
        return undefined;
    }
    return {
        async render(context: vscode.ExtensionContext): Promise<RenderedBody> {
            const rendererScript = await readJsonRendererScript(context);
            return {
                content: '<div id="json-body"></div>',
                script: `
        const jsonTree = JSON.parse(${JSON.stringify(serializeJsonTree(tree)).replace(/</g, '\\u003c')});

        ${rendererScript}

        renderJSON(jsonTree, document.getElementById('json-body'));
    `
            };
        }
    };
}

function htmlRenderer(mediaType: string | undefined, body: string): BodyRenderer | undefined {
    const isHtml = mediaType === undefined ? isHtmlDocument(body) : htmlMediaTypes.includes(mediaType);
    if (!isHtml) {
        return undefined;
    }
    return {
        async render(): Promise<RenderedBody> {
            return {
                content: `<div class="html-body">
            <div class="html-view-toggle">
                <button class="html-view-btn active" data-view="preview" onclick="showHtmlView('preview')">Preview</button>
                <button class="html-view-btn" data-view="source" onclick="showHtmlView('source')">Source</button>
            </div>
            <iframe class="html-view html-preview" data-view="preview" title="Response preview" sandbox="" srcdoc="${escapeHtml(body)}"></iframe>
            <div class="html-view html-source" data-view="source" hidden>${textBodyHtml('')}</div>
        </div>`,
                script: `
        function showHtmlView(view) {
            const preview = document.querySelector('.html-preview');
            const sourceCode = document.querySelector('.html-source code');
            if (view === 'preview') {
                preview.srcdoc = preview.srcdoc;
            } else if (!sourceCode.textContent) {
                sourceCode.textContent = preview.srcdoc;
            }
            document.querySelectorAll('.html-view-btn').forEach(button => button.classList.toggle('active', button.dataset.view === view));
            document.querySelectorAll('.html-view').forEach(element => { element.hidden = element.dataset.view !== view; });
        }
    `
            };
        }
    };
}

const textRenderer: BodyRenderer = {
    async render(_context: vscode.ExtensionContext, body: string): Promise<RenderedBody> {
        return {
            content: textBodyHtml(escapeHtml(body)),
            script: ''
        };
    }
};

export function binaryBodyRenderer(body: BinaryBody): BodyRenderer {
    const description = body.media_type ? `${body.media_type}, ${body.size} bytes` : `${body.size} bytes`;
    const notSaved = body.save_error === undefined ? 'Not saved' : `Could not be saved: ${escapeHtml(body.save_error)}`;
    const location = body.path === undefined
        ? `<p class="binary-location">${notSaved}</p>`
        : `<p class="binary-location">Saved to <code>${escapeHtml(body.path)}</code></p>
            <div class="binary-actions">
                <button class="html-view-btn active" onclick="vscode.postMessage({ command: 'openBody' })">Open</button>
                <button class="html-view-btn" onclick="vscode.postMessage({ command: 'revealBody' })">Reveal in file explorer</button>
            </div>`;
    return {
        async render(): Promise<RenderedBody> {
            return {
                content: `<div class="binary-body">
            <p class="binary-summary">Binary body (${escapeHtml(description)})</p>
            ${location}
        </div>`,
                script: ''
            };
        }
    };
}

function textBodyHtml(escapedText: string): string {
    return `<pre class="text-body"><code>${escapedText}</code></pre>`;
}

const mediaTypeRenderers: MediaTypeRenderer[] = [jsonRenderer, htmlRenderer];

let jsonRendererScript: Promise<string> | undefined;

export function getBodyRenderer(headers: Record<string, string>, body: string): BodyRenderer {
    const mediaType = mediaTypeOf(headers);
    for (const mediaTypeRenderer of mediaTypeRenderers) {
        const renderer = mediaTypeRenderer(mediaType, body);
        if (renderer !== undefined) {
            return renderer;
        }
    }
    return textRenderer;
}

function readJsonRendererScript(context: vscode.ExtensionContext): Promise<string> {
    jsonRendererScript ??= readMediaFile(context, 'jsonRenderer.js').catch((error: unknown) => {
        jsonRendererScript = undefined;
        throw error;
    });
    return jsonRendererScript;
}

async function readMediaFile(context: vscode.ExtensionContext, name: string): Promise<string> {
    const uri = vscode.Uri.joinPath(context.extensionUri, 'media', name);
    return new TextDecoder().decode(await vscode.workspace.fs.readFile(uri));
}

function mediaTypeOf(headers: Record<string, string>): string | undefined {
    const contentType = Object.entries(headers)
        .find(([name]) => name.toLowerCase() === 'content-type')?.[1];
    const mediaType = contentType?.split(';')[0].trim().toLowerCase();
    return mediaType ? mediaType : undefined;
}

const jsonMediaTypes = ['application/json', 'text/json', 'application/x-json'];

function isJsonMediaType(mediaType: string): boolean {
    return jsonMediaTypes.includes(mediaType) || mediaType.endsWith('+json');
}

const htmlMediaTypes = ['text/html', 'application/xhtml+xml'];

const htmlDocumentStart = /<!doctype\s+html|<html[\s>]/iy;
const leadingWhitespace = /\s*/y;
const leadingMarkup = [['<?xml', '?>'], ['<!--', '-->']];

function isHtmlDocument(body: string): boolean {
    let position = skipWhitespace(body, 0);
    let markupEnd = skipLeadingMarkup(body, position);
    while (markupEnd !== undefined) {
        position = skipWhitespace(body, markupEnd);
        markupEnd = skipLeadingMarkup(body, position);
    }
    htmlDocumentStart.lastIndex = position;
    return htmlDocumentStart.test(body);
}

function skipWhitespace(text: string, position: number): number {
    leadingWhitespace.lastIndex = position;
    leadingWhitespace.test(text);
    return leadingWhitespace.lastIndex;
}

function skipLeadingMarkup(text: string, position: number): number | undefined {
    const markup = leadingMarkup.find(([open]) => text.startsWith(open, position));
    if (markup === undefined) {
        return undefined;
    }
    const [open, close] = markup;
    const end = text.indexOf(close, position + open.length);
    return end < 0 ? undefined : end + close.length;
}
