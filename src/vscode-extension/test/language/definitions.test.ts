import { parseVariables } from '../../src/language/definitions';
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
