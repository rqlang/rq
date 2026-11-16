import { parseJsonTree } from '../src/ui/jsonTree';

describe('jsonTree', () => {
    test('keeps the text of a number', () => {
        const target = parseJsonTree('12345678901234567890');

        expect(target).toEqual(['12345678901234567890']);
    });

    test('keeps object entries in order, duplicates included', () => {
        const target = parseJsonTree('{"b":1,"2":0,"b":2}');

        expect(target).toEqual(['o', 'b', ['1'], '2', ['0'], 'b', ['2']]);
    });

    test('decodes string escapes', () => {
        const target = parseJsonTree('"a\\n\\u00e9\\""');

        expect(target).toEqual('a\né"');
    });

    test('parses arrays, literals and whitespace', () => {
        const target = parseJsonTree(' [ true , false , null , [ ] , { } ] \n');

        expect(target).toEqual(['a', ['true'], ['false'], ['null'], ['a'], ['o']]);
    });

    test('parses nested containers', () => {
        const target = parseJsonTree('{"a":[1,{"b":[]}],"c":{}}');

        expect(target).toEqual(['o', 'a', ['a', ['1'], ['o', 'b', ['a']]], 'c', ['o']]);
    });

    test('parses a string value of tens of megabytes', () => {
        const value = 'x'.repeat(20_000_000) + '\\n';

        const target = parseJsonTree(`{"file":"${value}"}`);

        expect(target).toEqual(['o', 'file', 'x'.repeat(20_000_000) + '\n']);
    });

    test('parses nesting deeper than a recursive parser can', () => {
        const depth = 200_000;

        const target = parseJsonTree('['.repeat(depth) + ']'.repeat(depth));

        expect(target).toBeDefined();
    });

    test.each([
        '',
        ' ',
        '{',
        '[1,]',
        '{"a":1,}',
        '{"a" 1}',
        '{a:1}',
        '01',
        '1.',
        '.5',
        '+1',
        '-',
        'NaN',
        'True',
        '"unterminated',
        '"\\x"',
        '"tab\there"',
        "'a'",
        '1 2',
        '[,]',
        '{,}',
        '[1 2]',
        '{"a":1 "b":2}',
        '[1}',
        '{"a":1]',
        '[[]',
        '[]]',
        '\u00a01',
        '\ufeff1'
    ])('rejects %j as JSON.parse does', (body) => {
        expect(() => JSON.parse(body)).toThrow();
        expect(parseJsonTree(body)).toBeUndefined();
    });
});
