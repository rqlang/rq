import { insideJsonLiteral } from '../../src/language/completionHelpers';

describe('insideJsonLiteral', () => {
    test('returns false for empty string', () => {
        expect(insideJsonLiteral('')).toBe(false);
    });

    test('returns true when cursor is inside open ${ }', () => {
        expect(insideJsonLiteral('${\n    ')).toBe(true);
    });

    test('returns false after ${ } is fully closed', () => {
        expect(insideJsonLiteral('${\n    "key": "value"\n}')).toBe(false);
    });

    test('returns false for plain { without $ prefix', () => {
        expect(insideJsonLiteral('{\n    ')).toBe(false);
    });

    test('returns true when inside nested { } within ${ }', () => {
        expect(insideJsonLiteral('${\n    "nested": {\n        ')).toBe(true);
    });

    test('returns true when outer ${ } still open after inner { } closes', () => {
        expect(insideJsonLiteral('${\n    "nested": { "a": 1 }\n    ')).toBe(true);
    });

    test('returns false before any ${ }', () => {
        expect(insideJsonLiteral('rq x(\n    ')).toBe(false);
    });

    test('returns false when $[...] precedes cursor but no ${', () => {
        expect(insideJsonLiteral('rq x(\n    $[\n        "Accept": "v"\n    ],\n    ')).toBe(false);
    });

    test('returns true when cursor is inside ${ } after a closed $[...]', () => {
        expect(insideJsonLiteral('rq x(\n    $[\n        "Accept": "v"\n    ],\n    ${\n        ')).toBe(true);
    });

    test('does not treat ${ inside a string as a json literal', () => {
        expect(insideJsonLiteral('"${var}"')).toBe(false);
    });

    test('handles escaped quotes inside strings', () => {
        expect(insideJsonLiteral('${\n    "key": "val with \\"quote\\""\n    ')).toBe(true);
    });
});
