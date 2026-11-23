import * as fs from 'fs';
import { Liquid, LiquidError } from 'liquidjs';
import type { FS } from 'liquidjs';
import type { RequestExecutionResult, ViewTarget } from '../execution';

export type ResponseView =
    | { kind: 'rendered'; document: string }
    | { kind: 'skipped'; reason: string }
    | { kind: 'failed'; message: string; template?: string; line?: number };

export interface ViewEnvironment {
    trusted: boolean;
    theme: 'light' | 'dark';
    stylesheet: string;
    readTemplate: (file: string) => Promise<string>;
}

const RENDER_LIMIT_MS = 2000;
const MEMORY_LIMIT = 10_000_000;

const VIEW_CSP = "default-src 'none'; style-src 'unsafe-inline'; img-src data:";

const noTemplateFiles: FS = {
    exists: async () => false,
    existsSync: () => false,
    readFile: async (file: string) => { throw new Error(`Views cannot load other templates: ${file}`); },
    readFileSync: (file: string) => { throw new Error(`Views cannot load other templates: ${file}`); },
    resolve: (_root: string, file: string) => file,
    contains: async () => false,
    sep: '/',
    dirname: () => ''
};

const liquid = new Liquid({
    outputEscape: 'escape',
    ownPropertyOnly: true,
    strictFilters: true,
    relativeReference: false,
    fs: noTemplateFiles,
    renderLimit: RENDER_LIMIT_MS,
    memoryLimit: MEMORY_LIMIT
});
delete liquid.filters.raw;

export async function renderResponseView(view: ViewTarget, result: RequestExecutionResult, environment: ViewEnvironment): Promise<ResponseView> {
    if (!environment.trusted) {
        return { kind: 'skipped', reason: 'views are not rendered in an untrusted workspace' };
    }
    const body = await jsonBodyOf(result);
    if ('reason' in body) {
        return { kind: 'skipped', reason: body.reason };
    }
    if (view.error) {
        return { kind: 'failed', message: view.error };
    }
    let template: string;
    try {
        template = await environment.readTemplate(view.path);
    } catch (error) {
        return { kind: 'failed', message: `View ${view.path} could not be read: ${messageOf(error)}` };
    }
    try {
        const rendered = await liquid.parseAndRender(template, viewContext(result, body.value));
        return { kind: 'rendered', document: viewDocument(rendered, environment) };
    } catch (error) {
        return { kind: 'failed', message: messageOf(error), template: view.path, line: errorLine(error) };
    }
}

async function jsonBodyOf(result: RequestExecutionResult): Promise<{ value: unknown } | { reason: string }> {
    if (result.status < 200 || result.status >= 300) {
        return { reason: `the response status is ${result.status}` };
    }
    if (result.body_truncated) {
        return { reason: 'the body was truncated' };
    }
    const text = await bodyTextOf(result);
    if (typeof text !== 'string') {
        return text;
    }
    try {
        return { value: JSON.parse(text) };
    } catch {
        return { reason: 'the body is not JSON' };
    }
}

async function bodyTextOf(result: RequestExecutionResult): Promise<string | { reason: string }> {
    const file = result.body_file;
    if (file === undefined) {
        return result.body;
    }
    if (file.binary || file.path === undefined) {
        return { reason: 'the body is binary' };
    }
    try {
        return await fs.promises.readFile(file.path, 'utf8');
    } catch (error) {
        return { reason: `the body file ${file.path} could not be read: ${messageOf(error)}` };
    }
}

function viewContext(result: RequestExecutionResult, body: unknown): Record<string, unknown> {
    return {
        body,
        status: result.status,
        headers: Object.fromEntries(Object.entries(result.response_headers).map(([name, value]) => [name.toLowerCase(), value])),
        request: { name: result.request_name, method: result.method, url: result.url }
    };
}

function viewDocument(rendered: string, environment: ViewEnvironment): string {
    return `<!DOCTYPE html>
<html data-theme="${environment.theme}">
<head>
<meta charset="UTF-8">
<meta http-equiv="Content-Security-Policy" content="${VIEW_CSP}">
<style>${environment.stylesheet}</style>
</head>
<body>
<main>
${rendered}
</main>
</body>
</html>`;
}

function errorLine(error: unknown): number | undefined {
    if (!(error instanceof LiquidError)) {
        return undefined;
    }
    const [line] = error.token.getPosition();
    return line;
}

function messageOf(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
}
