import * as fs from 'fs';
import * as path from 'path';
import { parseJsonTree } from '../src/ui/jsonTree';

interface FakeElement {
    className: string;
    textContent: string;
    innerHTML: string;
    style: Record<string, string>;
    children: FakeElement[];
    appendChild(child: FakeElement): void;
}

function createElement(): FakeElement {
    const element: FakeElement = {
        className: '',
        textContent: '',
        innerHTML: '',
        style: {},
        children: [],
        appendChild(child: FakeElement) {
            element.children.push(child);
        }
    };
    return element;
}

function allHtml(element: FakeElement): string {
    return element.innerHTML + element.children.map(allHtml).join('');
}

function renderJson(body: string): string {
    const source = fs.readFileSync(path.join(__dirname, '..', 'media', 'jsonRenderer.js'), 'utf8');
    const renderJSON = new Function('document', `${source}\nreturn renderJSON;`)({ createElement });
    const container = createElement();
    renderJSON(parseJsonTree(body), container);
    return allHtml(container);
}

describe('jsonRenderer', () => {
    test('escapes HTML in object keys', () => {
        const target = renderJson('{"<img src=x onerror=alert(1)>":1}');

        expect(target).not.toContain('<img');
        expect(target).toContain('&lt;img src=x onerror=alert(1)&gt;');
    });

    test('escapes HTML in string values', () => {
        const target = renderJson('{"a":"<b>x</b>"}');

        expect(target).toContain('&lt;b&gt;x&lt;/b&gt;');
    });

    test('shows an integer beyond the safe range as received', () => {
        const target = renderJson('{"id":12345678901234567890}');

        expect(target).toContain('<span class="json-number">12345678901234567890</span>');
    });

    test('shows integer-like keys in the received order', () => {
        const target = renderJson('{"b":1,"2":0,"1":0}');

        expect(target.indexOf('"b"')).toBeLessThan(target.indexOf('"2"'));
        expect(target.indexOf('"2"')).toBeLessThan(target.indexOf('"1"'));
    });

    test('shows every duplicate key', () => {
        const target = renderJson('{"a":1,"a":2}');

        expect(target.match(/<span class="json-key">"a"<\/span>/g)).toHaveLength(2);
    });

    test('shows a __proto__ key', () => {
        const target = renderJson('{"__proto__":{"x":1}}');

        expect(target).toContain('<span class="json-key">"__proto__"</span>');
    });

    test('shows a number in its received notation', () => {
        const target = renderJson('[1.50,1e3]');

        expect(target).toContain('<span class="json-number">1.50</span>');
        expect(target).toContain('<span class="json-number">1e3</span>');
    });
});
