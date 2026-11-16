import * as vscode from 'vscode';
import { escapeHtml } from './html';

export interface RenderedBody {
    content: string;
    script: string;
}

export interface BodyRenderer {
    accepts(mediaType: string | undefined, body: string): boolean;
    render(context: vscode.ExtensionContext, body: string): Promise<RenderedBody>;
}

const jsonRenderer: BodyRenderer = {
    accepts(mediaType: string | undefined, body: string): boolean {
        return (mediaType === undefined || isJsonMediaType(mediaType)) && parsesAsJson(body);
    },
    async render(context: vscode.ExtensionContext, body: string): Promise<RenderedBody> {
        const formattedBody = JSON.stringify(JSON.parse(body), null, 2).replace(/</g, '\\u003c');
        const rendererUri = vscode.Uri.joinPath(context.extensionUri, 'media', 'jsonRenderer.js');
        const rendererScript = new TextDecoder().decode(await vscode.workspace.fs.readFile(rendererUri));
        return {
            content: '<div id="json-body"></div>',
            script: `
        const jsonData = ${formattedBody};

        ${rendererScript}

        renderJSON(jsonData, document.getElementById('json-body'));
    `
        };
    }
};

const textRenderer: BodyRenderer = {
    accepts(): boolean {
        return true;
    },
    async render(_context: vscode.ExtensionContext, body: string): Promise<RenderedBody> {
        return {
            content: `<pre class="text-body"><code>${escapeHtml(body)}</code></pre>`,
            script: ''
        };
    }
};

const bodyRenderers: BodyRenderer[] = [jsonRenderer, textRenderer];

export function getBodyRenderer(headers: Record<string, string>, body: string): BodyRenderer {
    const mediaType = mediaTypeOf(headers);
    return bodyRenderers.find(renderer => renderer.accepts(mediaType, body)) ?? textRenderer;
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

function parsesAsJson(body: string): boolean {
    try {
        JSON.parse(body);
        return true;
    } catch {
        return false;
    }
}
