import * as vscode from 'vscode';
import { escapeHtml } from './html';

export interface RenderedBody {
    content: string;
    script: string;
}

export interface BodyRenderer {
    render(context: vscode.ExtensionContext, body: string): Promise<RenderedBody>;
}

interface MediaTypeRenderer extends BodyRenderer {
    accepts(mediaType: string | undefined, body: string): boolean;
}

const jsonRenderer: MediaTypeRenderer = {
    accepts(mediaType: string | undefined, body: string): boolean {
        return (mediaType === undefined || isJsonMediaType(mediaType)) && parsesAsJson(body);
    },
    async render(context: vscode.ExtensionContext, body: string): Promise<RenderedBody> {
        const formattedBody = JSON.stringify(JSON.parse(body), null, 2).replace(/</g, '\\u003c');
        const rendererScript = await readJsonRendererScript(context);
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
    async render(_context: vscode.ExtensionContext, body: string): Promise<RenderedBody> {
        return {
            content: `<pre class="text-body"><code>${escapeHtml(body)}</code></pre>`,
            script: ''
        };
    }
};

const mediaTypeRenderers: MediaTypeRenderer[] = [jsonRenderer];

let jsonRendererScript: Promise<string> | undefined;

export function getBodyRenderer(headers: Record<string, string>, body: string): BodyRenderer {
    const mediaType = mediaTypeOf(headers);
    return mediaTypeRenderers.find(renderer => renderer.accepts(mediaType, body)) ?? textRenderer;
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

function parsesAsJson(body: string): boolean {
    try {
        JSON.parse(body);
        return true;
    } catch {
        return false;
    }
}
