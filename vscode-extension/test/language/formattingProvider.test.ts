import * as vscode from 'vscode';
import { formatRqDocument } from '../../src/language/formattingProvider';

const fmt = (text: string) => formatRqDocument(text, 4);

function makeDocument(text: string) {
    const lines = text.split('\n');
    return {
        lineCount: lines.length,
        lineAt: (i: number) => ({ text: lines[i] }),
        getText: () => text
    };
}

let provideDocumentFormattingEdits: Function;

beforeAll(() => {
    const calls = (vscode.languages.registerDocumentFormattingEditProvider as jest.Mock).mock.calls;
    provideDocumentFormattingEdits = calls[calls.length - 1][1].provideDocumentFormattingEdits;
});

describe('formatRqDocument', () => {
    describe('indentation', () => {
        test('indents rq statements inside ep block', () => {
            expect(fmt('ep api("url") {\nrq list();\n}')).toBe('ep api("url") {\n    rq list();\n}\n');
        });

        test('indents auth block body', () => {
            expect(fmt('auth bearer(auth_type.bearer) {\ntoken: "xxx",\n}')).toBe('auth bearer(auth_type.bearer) {\n    token: "xxx",\n}\n');
        });

        test('indents env block body', () => {
            expect(fmt('env local {\nbase_url: "http://localhost",\n}')).toBe('env local {\n    base_url: "http://localhost",\n}\n');
        });

        test('corrects over-indented top-level line', () => {
            expect(fmt('    rq get("url");\n')).toBe('rq get("url");\n');
        });

        test('corrects under-indented line inside block', () => {
            expect(fmt('ep api("url") {\nrq list();\n}')).toBe('ep api("url") {\n    rq list();\n}\n');
        });

        test('respects custom tab size', () => {
            expect(formatRqDocument('ep api("url") {\nrq list();\n}', 2)).toBe('ep api("url") {\n  rq list();\n}\n');
        });

        test('handles nested block inside ep', () => {
            expect(fmt('ep api("url") {\n[method(POST)]\nrq create();\n}')).toBe('ep api("url") {\n    [method(POST)]\n    rq create();\n}\n');
        });
    });

    describe('blank lines between top-level statements', () => {
        test('does not insert blank between consecutive rq statements', () => {
            expect(fmt('rq get("url");\nrq post("url");')).toBe('rq get("url");\nrq post("url");\n');
        });

        test('preserves existing blank between top-level statements', () => {
            expect(fmt('rq get("url");\n\nrq post("url");')).toBe('rq get("url");\n\nrq post("url");\n');
        });

        test('collapses multiple blank lines to one', () => {
            expect(fmt('rq get("url");\n\n\n\nrq post("url");')).toBe('rq get("url");\n\nrq post("url");\n');
        });

        test('does not insert blank between let group and rq', () => {
            expect(fmt('let base_url = "http://localhost";\nrq get("{{base_url}}/users");')).toBe('let base_url = "http://localhost";\nrq get("{{base_url}}/users");\n');
        });

        test('inserts blank after closing brace before next statement', () => {
            expect(fmt('ep api("url") {\nrq list();\n}\nrq other("url");')).toBe('ep api("url") {\n    rq list();\n}\n\nrq other("url");\n');
        });

        test('does not insert blank before comment that precedes a statement', () => {
            expect(fmt('rq get("url");\n// comment\nrq post("url");')).toBe('rq get("url");\n// comment\nrq post("url");\n');
        });
    });

    describe('sticky pairs (no blank inserted)', () => {
        test('does not insert blank between attribute and rq', () => {
            expect(fmt('[method(POST)]\nrq create("url");')).toBe('[method(POST)]\nrq create("url");\n');
        });

        test('does not insert blank between consecutive attributes', () => {
            expect(fmt('[method(POST)]\n[auth("bearer")]\nrq create("url");')).toBe('[method(POST)]\n[auth("bearer")]\nrq create("url");\n');
        });

        test('does not insert blank between comment and following rq', () => {
            expect(fmt('// comment\nrq get("url");')).toBe('// comment\nrq get("url");\n');
        });

        test('does not insert blank between consecutive let statements', () => {
            expect(fmt('let base_url = "http://localhost";\nlet headers = ["Accept": "application/json"];')).toBe('let base_url = "http://localhost";\nlet headers = ["Accept": "application/json"];\n');
        });

        test('does not insert blank between consecutive import statements', () => {
            expect(fmt('import "shared";\nimport "auth";')).toBe('import "shared";\nimport "auth";\n');
        });

        test('does not insert blank before closing brace', () => {
            expect(fmt('ep api("url") {\n    rq list();\n}')).toBe('ep api("url") {\n    rq list();\n}\n');
        });
    });

    describe('blank line cleanup', () => {
        test('removes leading blank lines', () => {
            expect(fmt('\n\nrq get("url");')).toBe('rq get("url");\n');
        });

        test('removes trailing blank lines', () => {
            expect(fmt('rq get("url");\n\n\n')).toBe('rq get("url");\n');
        });

        test('preserves blank lines inside ep block', () => {
            expect(fmt('ep api("url") {\nrq list();\n\nrq get();\n}')).toBe('ep api("url") {\n    rq list();\n\n    rq get();\n}\n');
        });

        test('collapses multiple blanks inside block to one', () => {
            expect(fmt('ep api("url") {\nrq list();\n\n\nrq get();\n}')).toBe('ep api("url") {\n    rq list();\n\n    rq get();\n}\n');
        });
    });

    describe('multi-line array literals', () => {
        test('indents entries in multi-line array', () => {
            expect(fmt('let x = [\n"Accept": ""\n];')).toBe('let x = [\n    "Accept": ""\n];\n');
        });

        test('indents multiple entries in multi-line array', () => {
            expect(fmt('let x = [\n"Accept": "application/json",\n"Content-Type": "text/plain"\n];')).toBe('let x = [\n    "Accept": "application/json",\n    "Content-Type": "text/plain"\n];\n');
        });

        test('does not indent single-line array', () => {
            expect(fmt('let x = ["Accept": "application/json"];')).toBe('let x = ["Accept": "application/json"];\n');
        });

        test('does not insert blank before ]', () => {
            expect(fmt('let x = [\n"Accept": ""\n];')).not.toContain('\n\n');
        });

        test('splits ]; onto its own line when attached to last entry', () => {
            expect(fmt('let x = [\n    "Accept": "",\n    "pepito": ""];')).toBe('let x = [\n    "Accept": "",\n    "pepito": ""\n];\n');
        });

        test('splits multiple entries on same line onto individual lines', () => {
            expect(fmt('let x = [\n    "Accept": "", "pepito": "",\n    "Content-Type": ""\n];')).toBe('let x = [\n    "Accept": "",\n    "pepito": "",\n    "Content-Type": ""\n];\n');
        });

        test('joins ] and ); when on separate lines inside rq call', () => {
            expect(fmt('ep ep_name() {\n    rq my("", [\n        "hello": "",\n        "h": ""\n    ]\n    );\n}')).toBe('ep ep_name() {\n    rq my("", [\n        "hello": "",\n        "h": ""\n    ]);\n}\n');
        });

        test('keeps ], together when comma immediately follows ]', () => {
            expect(fmt('ep ep_name() {\n    rq my("", [\n        "h": ""\n    ],\n    "");\n}')).toBe('ep ep_name() {\n    rq my("", [\n        "h": ""\n    ],\n    "");\n}\n');
        });
    });

    describe('brace splitting', () => {
        test('splits content after { onto new line', () => {
            expect(fmt('ep api("url") {rq list();\n}')).toBe('ep api("url") {\n    rq list();\n}\n');
        });

        test('splits content before } onto previous line', () => {
            expect(fmt('ep api("url") {\nrq list();}')).toBe('ep api("url") {\n    rq list();\n}\n');
        });

        test('formats block entirely on one line', () => {
            expect(fmt('ep api("url") {rq list();}')).toBe('ep api("url") {\n    rq list();\n}\n');
        });

        test('does not split { inside string', () => {
            expect(fmt('let s = "a{b}";')).toBe('let s = "a{b}";\n');
        });

        test('does not split ${ in json body', () => {
            expect(fmt('rq post("url", ${"key": "val"});')).toBe('rq post("url", ${"key": "val"});\n');
        });

        test('indents multi-line json body and keeps }; together', () => {
            expect(fmt('let s = ${\n"a":"",\n"b":""\n};')).toBe('let s = ${\n    "a": "",\n    "b": ""\n};\n');
        });
    });

    describe('semicolon splitting', () => {
        test('splits two statements on the same line', () => {
            expect(fmt('rq get("url"); rq post("url");')).toBe('rq get("url");\nrq post("url");\n');
        });

        test('splits three statements on the same line', () => {
            expect(fmt('rq a("url"); rq b("url"); rq c("url");')).toBe('rq a("url");\nrq b("url");\nrq c("url");\n');
        });

        test('does not split ; inside string', () => {
            expect(fmt('let s = "a;b";')).toBe('let s = "a;b";\n');
        });
    });

    describe('trailing whitespace', () => {
        test('removes trailing spaces from lines', () => {
            expect(fmt('rq get("url");   \n')).toBe('rq get("url");\n');
        });

        test('removes trailing tabs from lines', () => {
            expect(fmt('rq get("url");\t\n')).toBe('rq get("url");\n');
        });
    });

    describe('trailing newline', () => {
        test('adds newline at end of file if missing', () => {
            expect(fmt('rq get("url");')).toBe('rq get("url");\n');
        });

        test('does not add extra newline when already present', () => {
            expect(fmt('rq get("url");\n')).toBe('rq get("url");\n');
        });
    });

    describe('spacing fixes', () => {
        test('removes space before ( in rq declaration', () => {
            expect(fmt('rq get ("url");')).toBe('rq get("url");\n');
        });

        test('normalizes multiple spaces before ( in rq declaration', () => {
            expect(fmt('rq rq_name    ("url");')).toBe('rq rq_name("url");\n');
        });

        test('removes space before ( in ep declaration', () => {
            expect(fmt('ep api ("url") {')).toBe('ep api("url") {\n');
        });

        test('removes space before ( in auth declaration', () => {
            expect(fmt('auth bearer (auth_type.bearer) {')).toBe('auth bearer(auth_type.bearer) {\n');
        });

        test('does not add space before ( in attribute', () => {
            expect(fmt('[method(POST)]')).toBe('[method(POST)]\n');
        });

        test('adds space before { in ep declaration', () => {
            expect(fmt('ep api("url"){')).toBe('ep api("url") {\n');
        });

        test('normalizes multiple spaces before { in ep declaration', () => {
            expect(fmt('ep api("url")   {')).toBe('ep api("url") {\n');
        });

        test('adds space before { in env declaration', () => {
            expect(fmt('env local{')).toBe('env local {\n');
        });

        test('joins { on its own line to previous line', () => {
            expect(fmt('ep api("url")\n{\nrq list();\n}')).toBe('ep api("url") {\n    rq list();\n}\n');
        });

        test('removes space before ;', () => {
            expect(fmt('rq get("url") ;')).toBe('rq get("url");\n');
        });

        test('removes multiple spaces before ;', () => {
            expect(fmt('rq get("url")   ;')).toBe('rq get("url");\n');
        });

        test('adds space after : in named parameter', () => {
            expect(fmt('rq post(body:"");')).toBe('rq post(body: "");\n');
        });

        test('normalizes extra space after : in named parameter', () => {
            expect(fmt('rq post(body:   "");')).toBe('rq post(body: "");\n');
        });

        test('adds space after : for bracket value', () => {
            expect(fmt('rq get(headers:[]);')).toBe('rq get(headers: []);\n');
        });

        test('does not alter : inside string value', () => {
            expect(fmt('let url = "http://example.com";')).toBe('let url = "http://example.com";\n');
        });

        test('does not alter port number after :', () => {
            expect(fmt('let url = "http://localhost:8080";')).toBe('let url = "http://localhost:8080";\n');
        });

        test('does not alter : inside quoted string content', () => {
            expect(fmt('let query = "category:books";')).toBe('let query = "category:books";\n');
        });

        test('adds space after , between arguments', () => {
            expect(fmt('rq post("url","body");')).toBe('rq post("url", "body");\n');
        });

        test('adds space after , in named args following string', () => {
            expect(fmt('rq post("url",body:"");')).toBe('rq post("url", body: "");\n');
        });

        test('does not alter , inside string', () => {
            expect(fmt('let s = "a,b,c";')).toBe('let s = "a,b,c";\n');
        });

        test('fixes colon spacing in header key-value pair', () => {
            expect(fmt('rq get("url", ["Content-Type":"application/json"]);')).toBe('rq get("url", ["Content-Type": "application/json"]);\n');
        });

        test('normalizes extra space before colon in header', () => {
            expect(fmt('rq get("url", ["Content-Type"  :  "application/json"]);')).toBe('rq get("url", ["Content-Type": "application/json"]);\n');
        });

        test('fixes missing space after comma between header entries', () => {
            expect(fmt('rq get("url", ["Accept": "application/json","Content-Type": "text/plain"]);')).toBe('rq get("url", ["Accept": "application/json", "Content-Type": "text/plain"]);\n');
        });

        test('does not alter already correct spacing', () => {
            expect(fmt('rq get("url", ["Content-Type": "application/json"]);\n')).toBe('rq get("url", ["Content-Type": "application/json"]);\n');
        });

        test('collapses multiple spaces after keyword to one', () => {
            expect(fmt('import    "_shared";')).toBe('import "_shared";\n');
        });

        test('collapses multiple spaces after rq keyword', () => {
            expect(fmt('rq    get("url");')).toBe('rq get("url");\n');
        });

        test('collapses multiple spaces after let keyword', () => {
            expect(fmt('let    base_url = "http://localhost";')).toBe('let base_url = "http://localhost";\n');
        });

        test('adds spaces around = in let assignment', () => {
            expect(fmt('let a="";')).toBe('let a = "";\n');
        });

        test('normalizes extra spaces around = in let assignment', () => {
            expect(fmt('let a  =  "";')).toBe('let a = "";\n');
        });

        test('does not alter = inside string value of let', () => {
            expect(fmt('let url="http://example.com?key=value";')).toBe('let url = "http://example.com?key=value";\n');
        });
    });

    describe('comments', () => {
        test('preserves single-line comments at top level', () => {
            expect(fmt('// comment\nrq get("url");')).toBe('// comment\nrq get("url");\n');
        });

        test('indents block comment inside ep', () => {
            expect(fmt('ep api("url") {\n/* comment */\nrq list();\n}')).toBe('ep api("url") {\n    /* comment */\n    rq list();\n}\n');
        });
    });

    describe('real-world documents', () => {
        test('formats a complete document', () => {
            const input = [
                'let base_url = "http://localhost:8080";',
                'let api_key = "secret";',
                '',
                '',
                'ep users("{{base_url}}/users", ["X-Api-Key":"{{api_key}}"]) {',
                'rq list();',
                'rq get();',
                '}',
                'auth bearer(auth_type.bearer) {',
                'token: "{{api_key}}",',
                '}',
            ].join('\n');

            const expected = [
                'let base_url = "http://localhost:8080";',
                'let api_key = "secret";',
                '',
                'ep users("{{base_url}}/users", ["X-Api-Key": "{{api_key}}"]) {',
                '    rq list();',
                '    rq get();',
                '}',
                '',
                'auth bearer(auth_type.bearer) {',
                '    token: "{{api_key}}",',
                '}',
                '',
            ].join('\n');

            expect(fmt(input)).toBe(expected);
        });

        test('formats rq with multiple named params', () => {
            expect(fmt('rq post(url:"http://example.com",body:"");')).toBe('rq post(url: "http://example.com", body: "");\n');
        });
    });
});

describe('formattingProvider', () => {
    test('registers document formatting provider for rq language', () => {
        const calls = (vscode.languages.registerDocumentFormattingEditProvider as jest.Mock).mock.calls;
        expect(calls.some(c => c[0] === 'rq')).toBe(true);
    });

    test('returns empty array when document is already formatted', () => {
        const text = 'rq get("url");\n';
        const doc = makeDocument(text);
        const result = provideDocumentFormattingEdits(doc, { tabSize: 4, insertSpaces: true });
        expect(result).toHaveLength(0);
    });

    test('returns a TextEdit replacing the full document when formatting is needed', () => {
        const text = 'rq get("url");\nrq post("url");';
        const doc = makeDocument(text);
        const result = provideDocumentFormattingEdits(doc, { tabSize: 4, insertSpaces: true });
        expect(result).toHaveLength(1);
        expect(result[0].newText).toBe('rq get("url");\nrq post("url");\n');
    });
});
