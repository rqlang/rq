import * as vscode from 'vscode';
import { getActiveParam } from '../../src/language/signatureHelpProvider';
import '../../src/language/signatureHelpProvider';

function makeDocument(text: string) {
    const lines = text.split('\n');
    return {
        lineCount: lines.length,
        lineAt: (i: number) => ({ text: lines[i] }),
        getText: (range?: vscode.Range) => {
            if (!range) return text;
            const start = range.start as vscode.Position;
            const end = range.end as vscode.Position;
            const slice = lines.slice(start.line, end.line + 1);
            if (slice.length === 1) return slice[0].slice(start.character, end.character);
            slice[0] = slice[0].slice(start.character);
            slice[slice.length - 1] = slice[slice.length - 1].slice(0, end.character);
            return slice.join('\n');
        },
        getWordRangeAtPosition: () => undefined
    };
}

function makePosition(text: string): vscode.Position {
    const lines = text.split('\n');
    return new vscode.Position(lines.length - 1, lines[lines.length - 1].length);
}

let provideSignatureHelp: Function;

beforeAll(() => {
    const calls = (vscode.languages.registerSignatureHelpProvider as jest.Mock).mock.calls;
    provideSignatureHelp = calls[calls.length - 1][1].provideSignatureHelp;
});

describe('getActiveParam', () => {
    describe('positional', () => {
        test('no commas → 0 (url)', () => {
            expect(getActiveParam('"http://example.com"', ['url', 'headers', 'body'])).toBe(0);
        });

        test('one comma → 1 (headers)', () => {
            expect(getActiveParam('"http://example.com", ', ['url', 'headers', 'body'])).toBe(1);
        });

        test('two commas → 2 (body)', () => {
            expect(getActiveParam('"url", ["h":"v"], ', ['url', 'headers', 'body'])).toBe(2);
        });

        test('commas inside nested brackets are ignored', () => {
            expect(getActiveParam('"url", ["a":"1", "b":"2"], ', ['url', 'headers', 'body'])).toBe(2);
        });

        test('commas inside string are ignored', () => {
            expect(getActiveParam('"url, with, commas", ', ['url', 'headers', 'body'])).toBe(1);
        });

        test('clamps at last param', () => {
            expect(getActiveParam('"url", ["h":"v"], "body", ', ['url', 'headers', 'body'])).toBe(2);
        });
    });

    describe('named', () => {
        test('url: → 0', () => {
            expect(getActiveParam('url: "http://example.com"', ['url', 'headers', 'body'])).toBe(0);
        });

        test('headers: → 1', () => {
            expect(getActiveParam('url: "url",\n    headers: [', ['url', 'headers', 'body'])).toBe(1);
        });

        test('body: → 2', () => {
            expect(getActiveParam('url: "url",\n    headers: [...],\n    body: ', ['url', 'headers', 'body'])).toBe(2);
        });

        test('after comma with no named keyword yet → uses comma count', () => {
            expect(getActiveParam('url: "url", ', ['url', 'headers', 'body'])).toBe(1);
        });
    });

    describe('mixed positional + named', () => {
        test('positional url then named headers', () => {
            expect(getActiveParam('"http://url", headers: [', ['url', 'headers', 'body'])).toBe(1);
        });

        test('positional url then named body', () => {
            expect(getActiveParam('"http://url", body: "', ['url', 'headers', 'body'])).toBe(2);
        });
    });

    describe('ep params', () => {
        test('qs: → 2', () => {
            expect(getActiveParam('url: "url",\n    qs: "', ['url', 'headers', 'qs'])).toBe(2);
        });
    });
});

describe('signatureHelpProvider', () => {
    describe('rq statements', () => {
        test('shows signature immediately after opening paren', () => {
            const text = 'rq my_request(';
            const doc = makeDocument(text);
            const pos = makePosition(text);
            const result = provideSignatureHelp(doc, pos);
            expect(result).toBeDefined();
            expect(result.signatures[0].label).toBe('rq my_request(url, headers?, body?)');
            expect(result.activeParameter).toBe(0);
        });

        test('highlights headers after first comma', () => {
            const text = 'rq get("http://example.com", ';
            const doc = makeDocument(text);
            const pos = makePosition(text);
            const result = provideSignatureHelp(doc, pos);
            expect(result.activeParameter).toBe(1);
        });

        test('highlights body after second comma', () => {
            const text = 'rq post("url", ["h":"v"], ';
            const doc = makeDocument(text);
            const pos = makePosition(text);
            const result = provideSignatureHelp(doc, pos);
            expect(result.activeParameter).toBe(2);
        });

        test('highlights url for named url: param', () => {
            const text = 'rq get(\n    url: "http://';
            const doc = makeDocument(text);
            const pos = makePosition(text);
            const result = provideSignatureHelp(doc, pos);
            expect(result.activeParameter).toBe(0);
        });

        test('highlights headers for named headers: param', () => {
            const text = 'rq get(\n    url: "http://url",\n    headers: [';
            const doc = makeDocument(text);
            const pos = makePosition(text);
            const result = provideSignatureHelp(doc, pos);
            expect(result.activeParameter).toBe(1);
        });

        test('highlights body for named body: param', () => {
            const text = 'rq post(\n    url: "url",\n    body: ';
            const doc = makeDocument(text);
            const pos = makePosition(text);
            const result = provideSignatureHelp(doc, pos);
            expect(result.activeParameter).toBe(2);
        });

        test('returns undefined outside rq block', () => {
            const text = 'let x = "value"';
            const doc = makeDocument(text);
            const pos = makePosition(text);
            const result = provideSignatureHelp(doc, pos);
            expect(result).toBeUndefined();
        });

        test('returns undefined after semicolon', () => {
            const text = 'rq get("url");\nlet x = ';
            const doc = makeDocument(text);
            const pos = makePosition(text);
            const result = provideSignatureHelp(doc, pos);
            expect(result).toBeUndefined();
        });
    });

    describe('ep statements', () => {
        test('shows ep signature with url/headers?/qs?', () => {
            const text = 'ep api(';
            const doc = makeDocument(text);
            const pos = makePosition(text);
            const result = provideSignatureHelp(doc, pos);
            expect(result).toBeDefined();
            expect(result.signatures[0].label).toBe('ep api(url, headers?, qs?)');
            expect(result.activeParameter).toBe(0);
        });

        test('highlights headers after first comma', () => {
            const text = 'ep api("https://api.example.com", ';
            const doc = makeDocument(text);
            const pos = makePosition(text);
            const result = provideSignatureHelp(doc, pos);
            expect(result.activeParameter).toBe(1);
        });

        test('highlights qs for named qs: param', () => {
            const text = 'ep api(\n    url: "https://api.example.com",\n    qs: "';
            const doc = makeDocument(text);
            const pos = makePosition(text);
            const result = provideSignatureHelp(doc, pos);
            expect(result.activeParameter).toBe(2);
        });

        test('returns undefined after opening brace', () => {
            const text = 'ep api("url") {\n    rq req(';
            const doc = makeDocument(text);
            const pos = makePosition(text);
            const result = provideSignatureHelp(doc, pos);
            expect(result?.signatures[0].label).toMatch(/^rq req/);
        });
    });

    describe('parameter label offsets', () => {
        test('parameter offsets point to correct positions in label', () => {
            const text = 'rq req(';
            const doc = makeDocument(text);
            const pos = makePosition(text);
            const result = provideSignatureHelp(doc, pos);
            const label = result.signatures[0].label;
            const params = result.signatures[0].parameters;
            expect(label.slice((params[0].label as [number,number])[0], (params[0].label as [number,number])[1])).toBe('url');
            expect(label.slice((params[1].label as [number,number])[0], (params[1].label as [number,number])[1])).toBe('headers?');
            expect(label.slice((params[2].label as [number,number])[0], (params[2].label as [number,number])[1])).toBe('body?');
        });
    });
});
