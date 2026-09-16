import { parseVariables } from '../../src/language/definitions';

import { splitArguments, assignArguments, extractArgumentList } from '../../src/language/definitions';

describe('extractArgumentList', () => {
    test('returns the text between the outer parens', () => {
        expect(extractArgumentList('rq a("u", "b");', 4)).toBe('"u", "b"');
    });

    test('ignores brackets that live inside a string', () => {
        expect(extractArgumentList('rq a("a)b");', 4)).toBe('"a)b"');
    });

    test('returns everything left when the list is never closed', () => {
        expect(extractArgumentList('rq a("u",', 4)).toBe('"u",');
    });

    test('spans several lines', () => {
        expect(extractArgumentList('rq a(\n  "u",\n  "h"\n)', 4)).toBe('\n  "u",\n  "h"\n');
    });
});

describe('splitArguments', () => {
    test('splits top-level segments only', () => {
        expect(splitArguments('"u", $["A": "1", "B": "2"], ${"k": 1}')).toEqual([
            '"u"', '$["A": "1", "B": "2"]', '${"k": 1}'
        ]);
    });

    test('ignores a comma inside a string', () => {
        expect(splitArguments('"a,b", "c"')).toEqual(['"a,b"', '"c"']);
    });

    test('drops a trailing comma', () => {
        expect(splitArguments('"u", ')).toEqual(['"u"']);
    });

    test('returns an empty list for an empty argument list', () => {
        expect(splitArguments('   ')).toEqual([]);
    });
});

describe('assignArguments', () => {
    const params = ['url', 'headers', 'body'];

    test('maps positional arguments in order', () => {
        const result = assignArguments('"u", $[], ${}', params);
        expect(result.get('url')).toBe('"u"');
        expect(result.get('headers')).toBe('$[]');
        expect(result.get('body')).toBe('${}');
    });

    test('maps named arguments regardless of order', () => {
        const result = assignArguments('body: "b", url: "u"', params);
        expect(result.get('url')).toBe('"u"');
        expect(result.get('body')).toBe('"b"');
        expect(result.has('headers')).toBe(false);
    });

    test('mixes positional and named arguments', () => {
        const result = assignArguments('"u", headers: $["A": "1"]', params);
        expect(result.get('url')).toBe('"u"');
        expect(result.get('headers')).toBe('$["A": "1"]');
    });

    test('does not treat a header entry as a named argument', () => {
        const result = assignArguments('"u", $["url": "1"]', params);
        expect(result.get('url')).toBe('"u"');
        expect(result.get('headers')).toBe('$["url": "1"]');
    });

    test('leaves unspecified parameters out of the map', () => {
        expect(assignArguments('"u"', params).size).toBe(1);
    });
});
import type * as vscode from 'vscode';

function makeDocument(lines: string[]): vscode.TextDocument {
    return {
        lineCount: lines.length,
        lineAt: (i: number) => ({ text: lines[i] })
    } as unknown as vscode.TextDocument;
}

describe('parseVariables', () => {
    test('finds a single let declaration', () => {
        const target = makeDocument([
            'let my_url = "http://example.com";'
        ]);
        const result = parseVariables(target);
        expect(result).toHaveLength(1);
        expect(result[0].name).toBe('my_url');
        expect(result[0].value).toBe('"http://example.com";');
        expect(result[0].line).toBe(0);
    });

    test('returns correct line numbers with imports and empty lines', () => {
        const target = makeDocument([
            'import "_shared";',
            '',
            'let d = datetime.now();',
            'let f = "value";',
            '',
            'rq get();',
            '',
            '',
            'let my_url = "http://example.com";'
        ]);
        const result = parseVariables(target);
        expect(result).toHaveLength(3);
        expect(result.find(v => v.name === 'my_url')?.line).toBe(8);
        expect(result.find(v => v.name === 'd')?.line).toBe(2);
        expect(result.find(v => v.name === 'f')?.line).toBe(3);
    });

    test('does not match env block keys', () => {
        const target = makeDocument([
            'env local {',
            '    api_url: "http://localhost:8080",',
            '}',
            'let api_url = "prod";'
        ]);
        const result = parseVariables(target);
        expect(result).toHaveLength(1);
        expect(result[0].name).toBe('api_url');
        expect(result[0].line).toBe(3);
    });

    test('handles multiple variables', () => {
        const target = makeDocument([
            'let a = "1";',
            'let b = "2";',
            'let c = "3";'
        ]);
        const result = parseVariables(target);
        expect(result).toHaveLength(3);
        expect(result[0]).toMatchObject({ name: 'a', line: 0 });
        expect(result[1]).toMatchObject({ name: 'b', line: 1 });
        expect(result[2]).toMatchObject({ name: 'c', line: 2 });
    });

    test('handles indented let declarations', () => {
        const target = makeDocument([
            'ep my_ep() {',
            '    let x = "val";',
            '}'
        ]);
        const result = parseVariables(target);
        expect(result).toHaveLength(1);
        expect(result[0]).toMatchObject({ name: 'x', line: 1 });
    });

    test('returns empty array for document with no variables', () => {
        const target = makeDocument([
            'import "_shared";',
            '',
            'rq get("http://example.com");'
        ]);
        expect(parseVariables(target)).toHaveLength(0);
    });
});
