import * as vscode from 'vscode';
import { getBodyRenderer } from '../src/ui/bodyRenderers';

describe('bodyRenderers', () => {
    const mockContext = {
        extensionUri: { fsPath: '/mock/extension' }
    } as unknown as vscode.ExtensionContext;

    beforeAll(() => {
        (vscode.workspace as any).fs = {
            readFile: jest.fn().mockResolvedValue(new TextEncoder().encode('function renderJSON() {}'))
        };
        (vscode.Uri as any).joinPath = jest.fn((base, ...segments) => ({
            fsPath: `${base.fsPath}/${segments.join('/')}`
        }));
    });

    const render = (headers: Record<string, string>, body: string) =>
        getBodyRenderer(headers, body).render(mockContext, body);

    test('renders a text/plain body that looks like JSON as text', async () => {
        const target = await render({ 'content-type': 'text/plain' }, '42');

        expect(target.content).toBe('<pre class="text-body"><code>42</code></pre>');
        expect(target.script).toBe('');
    });

    test('renders an application/json body as a JSON tree', async () => {
        const target = await render({ 'content-type': 'application/json' }, '{"a":1}');

        expect(target.content).toContain('id="json-body"');
        expect(target.script).toContain('const jsonTree = ["o","a",["1"]];');
    });

    test('embeds an object key named __proto__ as a string', async () => {
        const target = await render({ 'content-type': 'application/json' }, '{"__proto__":1}');

        expect(target.script).toContain('const jsonTree = ["o","__proto__",["1"]];');
    });

    test('keeps a closing script tag in a JSON body inside the script', async () => {
        const target = await render({ 'content-type': 'application/json' }, '{"a":"</script><script>x()</script>"}');

        expect(target.script).not.toContain('</script>');
        expect(target.script).toContain('"\\u003c/script>\\u003cscript>x()\\u003c/script>"');
    });

    test('renders a +json media type as a JSON tree', async () => {
        const target = await render({ 'content-type': 'application/problem+json' }, '{"title":"x"}');

        expect(target.content).toContain('id="json-body"');
    });

    test.each(['text/json', 'application/x-json'])('renders a %s body as a JSON tree', async (mediaType) => {
        const target = await render({ 'content-type': mediaType }, '{"a":1}');

        expect(target.content).toContain('id="json-body"');
    });

    test('ignores media type parameters and header name case', async () => {
        const target = await render({ 'Content-Type': 'Application/JSON; charset=utf-8' }, '{"a":1}');

        expect(target.content).toContain('id="json-body"');
    });

    test('renders an application/json body that does not parse as text', async () => {
        const target = await render({ 'content-type': 'application/json' }, '{not json');

        expect(target.content).toBe('<pre class="text-body"><code>{not json</code></pre>');
    });

    test('renders a body without content-type as JSON when it parses', async () => {
        const target = await render({}, '[1,2]');

        expect(target.content).toContain('id="json-body"');
    });

    test('renders a body without content-type as text when it does not parse', async () => {
        const target = await render({}, 'hello');

        expect(target.content).toBe('<pre class="text-body"><code>hello</code></pre>');
    });

    test('renders an unknown media type as text', async () => {
        const target = await render({ 'content-type': 'application/xml' }, '{"a":1}');

        expect(target.content).toBe('<pre class="text-body"><code>{&quot;a&quot;:1}</code></pre>');
    });

    test('embeds a JSON tree no larger than twice the body', async () => {
        const body = JSON.stringify(Array.from({ length: 1000 }, (_, index) => ({ id: index, name: `n${index}`, ok: true })));

        const target = await render({ 'content-type': 'application/json' }, body);

        expect(target.script.length).toBeLessThan(2 * body.length);
    });

    test('reads the JSON renderer script once', async () => {
        const readFile = jest.fn().mockResolvedValue(new TextEncoder().encode('function renderJSON() {}'));
        let isolatedGetBodyRenderer: typeof getBodyRenderer = getBodyRenderer;
        jest.isolateModules(() => {
            const isolatedVscode = require('vscode');
            isolatedVscode.workspace.fs = { readFile };
            isolatedVscode.Uri.joinPath = (vscode.Uri as any).joinPath;
            isolatedGetBodyRenderer = require('../src/ui/bodyRenderers').getBodyRenderer;
        });

        await isolatedGetBodyRenderer({}, '{}').render(mockContext, '{}');
        await isolatedGetBodyRenderer({}, '[]').render(mockContext, '[]');

        expect(readFile).toHaveBeenCalledTimes(1);
    });

    test('escapes HTML in a text body', async () => {
        const target = await render({ 'content-type': 'text/plain' }, '<b>hi</b>');

        expect(target.content).toContain('&lt;b&gt;hi&lt;/b&gt;');
    });
});
