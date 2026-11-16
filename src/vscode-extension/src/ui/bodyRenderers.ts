import * as vscode from 'vscode';
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
            const escapedBody = escapeHtml(body);
            return {
                content: `<div class="html-body">
            <div class="html-view-toggle">
                <button class="html-view-btn active" data-view="preview" onclick="showHtmlView('preview')">Preview</button>
                <button class="html-view-btn" data-view="source" onclick="showHtmlView('source')">Source</button>
            </div>
            <iframe class="html-view html-preview" data-view="preview" sandbox="" srcdoc="${escapedBody}"></iframe>
            <pre class="html-view text-body" data-view="source" hidden><code>${escapedBody}</code></pre>
        </div>`,
                script: `
        function showHtmlView(view) {
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
            content: `<pre class="text-body"><code>${escapeHtml(body)}</code></pre>`,
            script: ''
        };
    }
};

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

function isHtmlDocument(body: string): boolean {
    return /^(<!doctype\s+html|<html[\s>])/i.test(body.trimStart());
}
