import * as fs from 'fs';
import * as path from 'path';

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

function renderJson(data: unknown): string {
    const source = fs.readFileSync(path.join(__dirname, '..', 'media', 'jsonRenderer.js'), 'utf8');
    const renderJSON = new Function('document', `${source}\nreturn renderJSON;`)({ createElement });
    const container = createElement();
    renderJSON(data, container);
    return allHtml(container);
}

describe('jsonRenderer', () => {
    test('escapes HTML in object keys', () => {
        const target = renderJson({ '<img src=x onerror=alert(1)>': 1 });

        expect(target).not.toContain('<img');
        expect(target).toContain('&lt;img src=x onerror=alert(1)&gt;');
    });

    test('escapes HTML in string values', () => {
        const target = renderJson({ a: '<b>x</b>' });

        expect(target).toContain('&lt;b&gt;x&lt;/b&gt;');
    });
});
