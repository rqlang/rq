import { parseJsonTree } from '../src/ui/jsonTree';

describe('jsonTree', () => {
    test('keeps the text of a number', () => {
        const target = parseJsonTree('12345678901234567890');

        expect(target).toEqual({ type: 'number', text: '12345678901234567890' });
    });

    test('keeps object entries in order, duplicates included', () => {
        const target = parseJsonTree('{"b":1,"2":0,"b":2}');

        expect(target).toEqual({
            type: 'object',
            entries: [
                ['b', { type: 'number', text: '1' }],
                ['2', { type: 'number', text: '0' }],
                ['b', { type: 'number', text: '2' }]
            ]
        });
    });

    test('decodes string escapes', () => {
        const target = parseJsonTree('"a\\n\\u00e9\\""');

        expect(target).toEqual({ type: 'string', value: 'a\né"' });
    });

    test('parses arrays, literals and whitespace', () => {
        const target = parseJsonTree(' [ true , false , null , [ ] , { } ] \n');

        expect(target).toEqual({
            type: 'array',
            items: [
                { type: 'boolean', text: 'true' },
                { type: 'boolean', text: 'false' },
                { type: 'null', text: 'null' },
                { type: 'array', items: [] },
                { type: 'object', entries: [] }
            ]
        });
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
        ' 1',
        '﻿1'
    ])('rejects %j as JSON.parse does', (body) => {
        expect(() => JSON.parse(body)).toThrow();
        expect(parseJsonTree(body)).toBeUndefined();
    });

    test('rejects nesting too deep to walk', () => {
        const target = parseJsonTree('['.repeat(200000) + ']'.repeat(200000));

        expect(target).toBeUndefined();
    });
});
