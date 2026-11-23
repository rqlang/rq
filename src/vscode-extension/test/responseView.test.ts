import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import { renderResponseView, ResponseView, ViewEnvironment } from '../src/ui/responseView';
import type { RequestExecutionResult } from '../src/execution';

const ENVIRONMENT: ViewEnvironment = {
    trusted: true,
    theme: 'dark',
    stylesheet: 'main{}',
    readTemplate: file => fs.promises.readFile(file, 'utf8')
};

describe('renderResponseView', () => {
    let dir: string;

    beforeEach(() => {
        dir = fs.mkdtempSync(path.join(os.tmpdir(), 'rq-view-'));
    });

    afterEach(() => {
        fs.rmSync(dir, { recursive: true, force: true });
    });

    function viewFile(template: string): string {
        const file = path.join(dir, 'view.html');
        fs.writeFileSync(file, template);
        return file;
    }

    function response(overrides: Partial<RequestExecutionResult> = {}): RequestExecutionResult {
        return {
            request_name: 'list',
            method: 'GET',
            url: 'http://localhost/orders',
            status: 200,
            elapsed_ms: 1,
            request_headers: {},
            response_headers: { 'Content-Type': 'application/json' },
            body: '{"items":[{"id":1,"name":"<b>Ada</b>"}]}',
            ...overrides
        };
    }

    async function render(template: string, overrides: Partial<RequestExecutionResult> = {}, environment = ENVIRONMENT): Promise<ResponseView> {
        return renderResponseView({ path: viewFile(template) }, response(overrides), environment);
    }

    function documentOf(view: ResponseView): string {
        return view.kind === 'rendered' ? view.document : '';
    }

    test('refuses the raw filter, which would skip the escaping', async () => {
        const target = await render('{% if false %}{{ body.items[0].name | raw }}{% endif %}');
        expect(target.kind === 'failed' && target.message).toContain('undefined filter: raw');
    });

    test('renders the fields the template selects from the body', async () => {
        const target = await render('{% for item in body.items %}<td>{{ item.id }}</td>{% endfor %}');
        expect(documentOf(target)).toContain('<td>1</td>');
    });

    test('escapes markup that comes from the response', async () => {
        const target = await render('{{ body.items[0].name }}');
        expect(documentOf(target)).toContain('&lt;b&gt;Ada&lt;/b&gt;');
    });

    test('exposes the status, lowercase headers and request', async () => {
        const target = await render('{{ status }} {{ headers["content-type"] }} {{ request.name }} {{ request.method }} {{ request.url }}');
        expect(documentOf(target)).toContain('200 application/json list GET http://localhost/orders');
    });

    test('renders a missing field as empty', async () => {
        const target = await render('[{{ body.missing.field }}]');
        expect(documentOf(target)).toContain('[]');
    });

    test('wraps the view in a document with the theme, the stylesheet and a policy without scripts', async () => {
        const target = documentOf(await render('<p>ok</p>'));
        expect([target.includes('data-theme="dark"'), target.includes('<style>main{}</style>'), target.includes("default-src 'none'")]).toEqual([true, true, true]);
    });

    test('skips a response whose status is not 2xx', async () => {
        const target = await render('<p></p>', { status: 404 });
        expect(target).toEqual({ kind: 'skipped', reason: 'the response status is 404' });
    });

    test('skips a body that is not JSON', async () => {
        const target = await render('<p></p>', { body: 'plain text' });
        expect(target).toEqual({ kind: 'skipped', reason: 'the body is not JSON' });
    });

    test('skips a truncated body', async () => {
        const target = await render('<p></p>', { body_truncated: true });
        expect(target).toEqual({ kind: 'skipped', reason: 'the body was truncated' });
    });

    test('skips every view in an untrusted workspace', async () => {
        const target = await render('<p></p>', {}, { ...ENVIRONMENT, trusted: false });
        expect(target.kind).toBe('skipped');
    });

    test('reports the error that rq-lib found for the view without a template to open', async () => {
        const target = await renderResponseView({ path: '/nowhere.html', error: 'View /nowhere.html not found' }, response(), ENVIRONMENT);
        expect(target).toEqual({ kind: 'failed', message: 'View /nowhere.html not found' });
    });

    test('explains a non-2xx response before the error of the view', async () => {
        const target = await renderResponseView({ path: '/nowhere.html', error: 'View /nowhere.html not found' }, response({ status: 500 }), ENVIRONMENT);
        expect(target).toEqual({ kind: 'skipped', reason: 'the response status is 500' });
    });

    test('renders a JSON body that output wrote to a file', async () => {
        const bodyFile = path.join(dir, 'orders.json');
        fs.writeFileSync(bodyFile, '{"items":[{"id":3}]}');
        const target = await render('{{ body.items[0].id }}', {
            body: '',
            body_file: { media_type: 'application/json', file_name: 'orders.json', size: 20, binary: false, path: bodyFile }
        });
        expect(documentOf(target)).toContain('3');
    });

    test('renders again the body output wrote, even after the file is overwritten', async () => {
        const bodyFile = path.join(dir, 'orders.json');
        fs.writeFileSync(bodyFile, '{"id":1}');
        const result = response({
            body: '',
            body_file: { media_type: 'application/json', file_name: 'orders.json', size: 8, binary: false, path: bodyFile }
        });
        const view = { path: viewFile('{{ body.id }}') };
        await renderResponseView(view, result, ENVIRONMENT);
        fs.writeFileSync(bodyFile, '{"id":2}');

        const target = await renderResponseView(view, { ...result }, ENVIRONMENT);

        expect(documentOf(target)).toContain('<main>\n1\n</main>');
    });

    test('says when the file output wrote the body to cannot be read', async () => {
        const target = await render('<p></p>', {
            body: '',
            body_file: { media_type: 'application/json', file_name: 'gone.json', size: 2, binary: false, path: path.join(dir, 'gone.json') }
        });
        expect(target.kind === 'skipped' && target.reason).toContain('could not be read');
    });

    test('skips a binary body', async () => {
        const target = await render('<p></p>', {
            body: '',
            body_file: { media_type: 'image/png', file_name: 'logo.png', size: 4, binary: true, path: '/tmp/logo.png' }
        });
        expect(target).toEqual({ kind: 'skipped', reason: 'the body is binary' });
    });

    test('reports the line of a template syntax error', async () => {
        const target = await render('<p>ok</p>\n{% for item in %}');
        expect(target.kind === 'failed' && target.line).toBe(2);
    });

    test('names the template that failed so it can be opened', async () => {
        const target = await render('{% if %}');
        expect(target.kind === 'failed' && target.template).toBe(path.join(dir, 'view.html'));
    });

    test('reports an unknown filter', async () => {
        const target = await render('{{ body.items | no_such_filter }}');
        expect(target.kind === 'failed' && target.message).toContain('no_such_filter');
    });

    test('refuses to include another file', async () => {
        fs.writeFileSync(path.join(dir, 'other.html'), 'secret');
        const target = await render(`{% include "${path.join(dir, 'other.html')}" %}`);
        expect(target.kind).toBe('failed');
    });

    test('stops a template that allocates without bound', async () => {
        const target = await render('{% for i in (1..100000000) %}x{% endfor %}');
        expect(target.kind).toBe('failed');
    });
});
