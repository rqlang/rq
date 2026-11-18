import * as vscode from 'vscode';
import { bodyFileRenderer, getBodyRenderer } from '../src/ui/bodyRenderers';

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

    const embeddedTree = (script: string) =>
        new Function('document', `${script}\nreturn jsonTree;`)({ getElementById: () => null });

    test('describes a saved binary body with its media type, size, path and actions', async () => {
        const target = await bodyFileRenderer({ media_type: 'image/png', file_name: 'a.png', size: 3, binary: true, path: '/tmp/<id>/a.png' }).render(mockContext, '');

        expect(target.content).toContain('Binary body (image/png, 3 bytes)');
        expect(target.content).toContain('<code>/tmp/&lt;id&gt;/a.png</code>');
        expect(target.content).toContain("command: 'openBody'");
        expect(target.content).toContain("command: 'revealBody'");
    });

    test('describes a binary body without media type or path', async () => {
        const target = await bodyFileRenderer({ media_type: null, file_name: 'a.bin', size: 3, binary: true }).render(mockContext, '');

        expect(target.content).toContain('Binary body (3 bytes)');
        expect(target.content).toContain('Not saved');
        expect(target.content).not.toContain('openBody');
    });

    test('describes a text body written to its output without calling it binary', async () => {
        const target = await bodyFileRenderer({ media_type: 'application/json', file_name: 'users.json', size: 2, binary: false, path: '/repo/out/users.json' }).render(mockContext, '');

        expect([target.content.includes('Body (application/json, 2 bytes)'), target.content.includes('Binary body')]).toEqual([true, false]);
    });

    test('says why a binary body could not be saved', async () => {
        const target = await bodyFileRenderer({ media_type: 'image/png', file_name: 'a.png', size: 3, binary: true, save_error: 'EACCES <denied>' }).render(mockContext, '');

        expect(target.content).toContain('Could not be saved: EACCES &lt;denied&gt;');
        expect(target.content).not.toContain('openBody');
    });

    test('renders a text/plain body that looks like JSON as text', async () => {
        const target = await render({ 'content-type': 'text/plain' }, '42');

        expect(target.content).toBe('<pre class="text-body"><code>42</code></pre>');
        expect(target.script).toBe('');
    });

    test('renders an application/json body as a JSON tree', async () => {
        const target = await render({ 'content-type': 'application/json' }, '{"a":1}');

        expect(target.content).toContain('id="json-body"');
        expect(embeddedTree(target.script)).toEqual(['o', 'a', ['1']]);
    });

    test('embeds an object key named __proto__ as a string', async () => {
        const target = await render({ 'content-type': 'application/json' }, '{"__proto__":1}');

        expect(embeddedTree(target.script)).toEqual(['o', '__proto__', ['1']]);
    });

    test('keeps a closing script tag in a JSON body inside the script', async () => {
        const target = await render({ 'content-type': 'application/json' }, '{"a":"</script><script>x()</script>"}');

        expect(target.script).not.toContain('</script>');
        expect(embeddedTree(target.script)).toEqual(['o', 'a', '</script><script>x()</script>']);
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

    test('embeds a tree nested deeper than the call stack', async () => {
        const depth = 200_000;

        const target = await render({ 'content-type': 'application/json' }, '['.repeat(depth) + ']'.repeat(depth));

        let node = embeddedTree(target.script);
        for (let level = 1; level < depth; level++) {
            node = node[1];
        }
        expect(node).toEqual(['a']);
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

    test('renders a text/html body as a sandboxed preview', async () => {
        const target = await render({ 'content-type': 'text/html; charset=utf-8' }, '<p>hi</p>');

        expect(target.content).toContain('sandbox="" srcdoc="&lt;p&gt;hi&lt;/p&gt;"></iframe>');
    });

    test('renders an application/xhtml+xml body as a preview', async () => {
        const target = await render({ 'content-type': 'application/xhtml+xml' }, '<html></html>');

        expect(target.content).toContain('class="html-view html-preview"');
    });

    test('embeds the HTML body once', async () => {
        const target = await render({ 'content-type': 'text/html' }, '<p>hi</p>');

        expect(target.content.split('&lt;p&gt;hi&lt;/p&gt;')).toHaveLength(2);
    });

    test('labels the HTML preview frame', async () => {
        const target = await render({ 'content-type': 'text/html' }, '<p>hi</p>');

        expect(target.content).toContain('title="Response preview"');
    });

    test('escapes quotes in the preview srcdoc', async () => {
        const target = await render({ 'content-type': 'text/html' }, '<a href="x" onclick=\'y\'>');

        expect(target.content).toContain('srcdoc="&lt;a href=&quot;x&quot; onclick=&#039;y&#039;&gt;"');
    });

    test.each(['<!DOCTYPE html><html></html>', '  \n<html lang="en"></html>', '\uFEFF<html>', '<!-- generated -->\n<!DOCTYPE html><html>', '<?xml version="1.0"?>\n<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.0 Strict//EN"><html>'])('renders a body without content-type that is an HTML document as a preview: %j', async (body) => {
        const target = await render({}, body);

        expect(target.content).toContain('class="html-view html-preview"');
    });

    test('renders a body without content-type that only contains HTML tags as text', async () => {
        const target = await render({}, '<p>hi</p>');

        expect(target.content).toBe('<pre class="text-body"><code>&lt;p&gt;hi&lt;/p&gt;</code></pre>');
    });

    test.each(['<!-- a --><p>hi</p>', '<!-- unterminated <html>', '<?xml version="1.0"?><root/>'])('renders a body without content-type whose leading markup is not followed by an html tag as text: %j', async (body) => {
        const target = await render({}, body);

        expect(target.content).not.toContain('html-preview');
    });

    test('detects HTML after many leading comments in linear time', async () => {
        const body = '<!---->'.repeat(200_000) + 'x';

        const started = Date.now();
        const target = await render({}, body);

        expect(target.content).not.toContain('html-preview');
        expect(Date.now() - started).toBeLessThan(1000);
    });

    test('renders a text/plain HTML document as text', async () => {
        const target = await render({ 'content-type': 'text/plain' }, '<!doctype html><html></html>');

        expect(target.content).toBe('<pre class="text-body"><code>&lt;!doctype html&gt;&lt;html&gt;&lt;/html&gt;</code></pre>');
    });

    const htmlViewDocument = (srcdoc: string) => {
        const preview = { dataset: { view: 'preview' }, hidden: false, srcdoc, srcdocAssignments: 0 };
        const trackedPreview = new Proxy(preview, {
            set(target, property, value) {
                if (property === 'srcdoc') {
                    target.srcdocAssignments += 1;
                }
                return Reflect.set(target, property, value);
            }
        });
        const source = { dataset: { view: 'source' }, hidden: true };
        const sourceCode = { textContent: '' };
        const buttons = ['preview', 'source'].map(view => ({ dataset: { view }, classList: { toggle: jest.fn() } }));
        const document = {
            querySelector: (selector: string) => selector === '.html-preview' ? trackedPreview : sourceCode,
            querySelectorAll: (selector: string) => selector === '.html-view-btn' ? buttons : [trackedPreview, source]
        };
        return { document, preview, source, sourceCode, buttons };
    };

    const showHtmlView = async (view: string) => {
        const target = await render({ 'content-type': 'text/html' }, '<p>hi</p>');
        const page = htmlViewDocument('<p>hi</p>');
        new Function('document', `${target.script}\nshowHtmlView('${view}');`)(page.document);
        return page;
    };

    test('shows the HTML source and hides the preview', async () => {
        const target = await showHtmlView('source');

        expect([target.preview.hidden, target.source.hidden]).toEqual([true, false]);
        expect(target.buttons[1].classList.toggle).toHaveBeenCalledWith('active', true);
    });

    test('fills the HTML source from the preview on first show', async () => {
        const target = await showHtmlView('source');

        expect(target.sourceCode.textContent).toBe('<p>hi</p>');
    });

    test('reloads the response into the preview when it is shown again', async () => {
        const target = await showHtmlView('preview');

        expect(target.preview.srcdocAssignments).toBe(1);
        expect(target.preview.srcdoc).toBe('<p>hi</p>');
    });

    test('escapes HTML in a text body', async () => {
        const target = await render({ 'content-type': 'text/plain' }, '<b>hi</b>');

        expect(target.content).toContain('&lt;b&gt;hi&lt;/b&gt;');
    });
});
