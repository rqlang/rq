import * as fs from 'fs';
import * as path from 'path';
import { formatRqDocument } from '../../src/language/formattingProvider';

const repoRoot = path.resolve(__dirname, '../../../..');
const ignoredDirectories = new Set(['node_modules', 'target', 'out', '.git', 'wasm']);

function collectRqFiles(dir: string): string[] {
    const found: string[] = [];
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
        if (entry.isDirectory()) {
            if (ignoredDirectories.has(entry.name)) { continue; }
            found.push(...collectRqFiles(path.join(dir, entry.name)));
        } else if (entry.name.endsWith('.rq')) {
            found.push(path.join(dir, entry.name));
        }
    }
    return found;
}

const fixtures = collectRqFiles(repoRoot)
    .map(file => [path.relative(repoRoot, file), fs.readFileSync(file, 'utf8')] as const)
    .filter(([, text]) => text.trim().length > 0)
    .sort(([a], [b]) => a.localeCompare(b));

function contentSignature(text: string): string {
    const parts: string[] = [];
    let i = 0;
    while (i < text.length) {
        const ch = text[i];
        if (ch === '"' || ch === "'") {
            let j = i + 1;
            let closed = false;
            while (j < text.length) {
                if (text[j] === '\\') { j += 2; continue; }
                if (text[j] === ch) { j++; closed = true; break; }
                j++;
            }
            if (!closed) {
                const end = text.indexOf('\n', i);
                j = end === -1 ? text.length : end;
            }
            parts.push(text.slice(i, j).trimEnd());
            i = j;
            continue;
        }
        if (ch === '/' && text[i + 1] === '/') {
            const end = text.indexOf('\n', i);
            const stop = end === -1 ? text.length : end;
            parts.push(text.slice(i, stop).replace(/\s+/g, ' ').trimEnd());
            i = stop;
            continue;
        }
        if (ch === '/' && text[i + 1] === '*') {
            const end = text.indexOf('*/', i + 2);
            const stop = end === -1 ? text.length : end + 2;
            parts.push(text.slice(i, stop).replace(/\s+/g, ' '));
            i = stop;
            continue;
        }
        if (!/\s/.test(ch)) { parts.push(ch); }
        i++;
    }
    return parts.join('');
}

function createRandom(seed: number): () => number {
    let state = seed >>> 0;
    return () => {
        state = (state * 1664525 + 1013904223) >>> 0;
        return state / 0x100000000;
    };
}

const generatorLines = [
    'let base_url = "http://localhost:8080";',
    'let  token   =  "abc" ;',
    'import "./shared.rq";',
    'rq list(url: base_url);',
    'rq  get_one ( url : "${base_url}/1" , headers : my_headers ) ;',
    'ep users(url: base_url) {',
    'rq create(url: "/users",\nbody: "{}");',
    'env local {',
    'base_url: "http://localhost",',
    'auth api(auth_type.bearer) {',
    'token: "xxx",',
    '}',
    'let headers = $["Accept": "application/json", "X-Trace": "1"];',
    'let list = $[\n"a": "1",\n"b": "2"\n];',
    '// a trailing comment',
    '/* block\n * comment\n */',
    '',
    '   ',
    'rq nested(url: "u", qs: $["k": "v"]);',
];

function generateDocument(random: () => number): string {
    const lineCount = 1 + Math.floor(random() * 12);
    const picked: string[] = [];
    for (let i = 0; i < lineCount; i++) {
        const line = generatorLines[Math.floor(random() * generatorLines.length)];
        const padding = ' '.repeat(Math.floor(random() * 9));
        picked.push(padding + line);
    }
    return picked.join('\n');
}

describe('formatter properties', () => {
    test('the fixture corpus was discovered', () => {
        expect(fixtures.length).toBeGreaterThan(200);
    });

    describe('idempotence on the fixture corpus', () => {
        test.each(fixtures)('%s', (_name, text) => {
            const once = formatRqDocument(text, 4);
            expect(formatRqDocument(once, 4)).toBe(once);
        });
    });

    describe('content preservation on the fixture corpus', () => {
        test.each(fixtures)('%s', (_name, text) => {
            expect(contentSignature(formatRqDocument(text, 4))).toBe(contentSignature(text));
        });
    });

    describe('idempotence on generated documents', () => {
        const seeds = Array.from({ length: 500 }, (_, i) => i + 1);

        test.each(seeds)('seed %i', seed => {
            const document = generateDocument(createRandom(seed));
            const once = formatRqDocument(document, 4);
            expect(formatRqDocument(once, 4)).toBe(once);
        });
    });

    describe('content preservation on generated documents', () => {
        const seeds = Array.from({ length: 500 }, (_, i) => i + 1);

        test.each(seeds)('seed %i', seed => {
            const document = generateDocument(createRandom(seed));
            expect(contentSignature(formatRqDocument(document, 4))).toBe(contentSignature(document));
        });
    });

    describe('contentSignature', () => {
        test('ignores whitespace outside string literals', () => {
            expect(contentSignature('rq  a ( url :  "x" ) ;')).toBe('rqa(url:"x");');
        });

        test('keeps whitespace inside string literals', () => {
            expect(contentSignature('let a = "two  words";')).toBe('leta="two  words";');
        });

        test('keeps a quote inside a comment from opening a string', () => {
            expect(contentSignature("// don't\nrq a();")).toBe("// don'trqa();");
        });
    });
});
