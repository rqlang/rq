import * as fs from 'fs';
import { Liquid, LiquidError } from 'liquidjs';
import type { FS } from 'liquidjs';
import type { RequestExecutionResult, ViewTarget } from '../execution';

export type ResponseView =
    | { kind: 'rendered'; document: string }
    | { kind: 'skipped'; reason: string }
    | { kind: 'failed'; message: string; line?: number };

export interface ViewEnvironment {
    trusted: boolean;
    theme: 'light' | 'dark';
    stylesheet: string;
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

export async function renderResponseView(view: ViewTarget, result: RequestExecutionResult, environment: ViewEnvironment): Promise<ResponseView> {
    if (!environment.trusted) {
        return { kind: 'skipped', reason: 'views are not rendered in an untrusted workspace' };
    }
    if (view.error) {
        return { kind: 'failed', message: view.error };
    }
    const skipReason = skipReasonOf(result);
    if (skipReason !== undefined) {
        return { kind: 'skipped', reason: skipReason };
    }
    const body = parseJson(result.body);
    if (body === undefined) {
        return { kind: 'skipped', reason: 'the body is not JSON' };
    }
    let template: string;
    try {
        template = await fs.promises.readFile(view.path, 'utf8');
    } catch (error) {
        return { kind: 'failed', message: `View ${view.path} could not be read: ${messageOf(error)}` };
    }
    try {
        const rendered = await liquid.parseAndRender(template, viewContext(result, body.value));
        return { kind: 'rendered', document: viewDocument(rendered, environment) };
    } catch (error) {
        return { kind: 'failed', message: messageOf(error), line: errorLine(error) };
    }
}

function skipReasonOf(result: RequestExecutionResult): string | undefined {
    if (result.status < 200 || result.status >= 300) {
        return `the response status is ${result.status}`;
    }
    if (result.body_file) {
        return 'the body is not JSON';
    }
    if (result.body_truncated) {
        return 'the body was truncated';
    }
    return undefined;
}

function parseJson(text: string): { value: unknown } | undefined {
    try {
        return { value: JSON.parse(text) };
    } catch {
        return undefined;
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
