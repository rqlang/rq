import { insideJsonLiteral, insideUnclosedAttribute, getAuthAttributeContext } from '../../src/language/completionHelpers';

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

describe('getAuthAttributeContext', () => {
    test('returns null outside any attribute', () => {
        expect(getAuthAttributeContext('rq foo("http://x");\n')).toBeNull();
    });

    test('detects an empty quoted name', () => {
        expect(getAuthAttributeContext('[auth("')).toEqual({ quoted: true });
    });

    test('detects a name being typed between quotes', () => {
        expect(getAuthAttributeContext('[auth("my_be')).toEqual({ quoted: true });
    });

    test('detects the attribute before the opening quote', () => {
        expect(getAuthAttributeContext('[auth(')).toEqual({ quoted: false });
    });

    test('detects the attribute split across lines', () => {
        expect(getAuthAttributeContext('[auth(\n    ')).toEqual({ quoted: false });
    });

    test('detects the attribute with spaces around the name', () => {
        expect(getAuthAttributeContext('[ auth ("')).toEqual({ quoted: true });
    });

    test('detects the attribute when another attribute precedes it', () => {
        expect(getAuthAttributeContext('[timeout(30)] [auth("')).toEqual({ quoted: true });
    });

    test('returns null once the name is closed', () => {
        expect(getAuthAttributeContext('[auth("my_bearer"')).toBeNull();
    });

    test('returns null once the attribute is closed', () => {
        expect(getAuthAttributeContext('[auth("my_bearer")]\n')).toBeNull();
    });

    test('returns null inside a headers literal', () => {
        expect(getAuthAttributeContext('let h = $[auth("')).toBeNull();
    });

    test('returns null for another attribute', () => {
        expect(getAuthAttributeContext('[method(')).toBeNull();
    });

    test('returns null inside an array literal', () => {
        expect(getAuthAttributeContext('let values = [1, ')).toBeNull();
    });

    test('ignores apostrophes inside line comments', () => {
        expect(getAuthAttributeContext("// don't do this\n[auth(\"")).toEqual({ quoted: true });
    });

    test('ignores apostrophes inside block comments', () => {
        expect(getAuthAttributeContext("/* don't do this */\n[auth(\"")).toEqual({ quoted: true });
    });

    test('ignores escaped quotes inside strings', () => {
        expect(getAuthAttributeContext('rq a(body: "he said \\"hi")\n[auth("')).toEqual({ quoted: true });
    });

    test('detects a single quoted name', () => {
        expect(getAuthAttributeContext("[auth('my_be")).toEqual({ quoted: true });
    });

    test('returns null once a single quoted name is closed', () => {
        expect(getAuthAttributeContext("[auth('my_bearer'")).toBeNull();
    });

    test('returns null when the cursor is inside a line comment', () => {
        expect(getAuthAttributeContext('[auth("my_bearer")]\n// note: don\'t')).toBeNull();
    });
});

describe('insideUnclosedAttribute', () => {
    test('returns false for empty string', () => {
        expect(insideUnclosedAttribute('')).toBe(false);
    });

    test('returns true inside an unfinished auth attribute', () => {
        expect(insideUnclosedAttribute('[auth(\n    ')).toBe(true);
    });

    test('returns true inside an unfinished method attribute', () => {
        expect(insideUnclosedAttribute('[method(\n    ')).toBe(true);
    });

    test('returns false after the attribute is closed', () => {
        expect(insideUnclosedAttribute('[auth("my_bearer")]\n')).toBe(false);
    });

    test('returns false inside an array literal', () => {
        expect(insideUnclosedAttribute('let values = [1, ')).toBe(false);
    });

    test('returns false inside a headers literal', () => {
        expect(insideUnclosedAttribute('let h = $[')).toBe(false);
    });

    test('returns true when a comment above contains an apostrophe', () => {
        expect(insideUnclosedAttribute("// don't do this\n[auth(")).toBe(true);
    });

    test('returns false when the cursor is inside a line comment', () => {
        expect(insideUnclosedAttribute('[auth("x")]\n// note: don\'t')).toBe(false);
    });
});
