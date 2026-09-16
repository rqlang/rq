jest.mock('../../src/rqClient');

import * as vscode from 'vscode';
import * as cliService from '../../src/rqClient';
import { setEnvironmentProvider } from '../../src/language/hoverProvider';
import '../../src/language/hoverProvider';

function makeDocument(lines: string[]) {
    return {
        lineCount: lines.length,
        lineAt: (i: number | vscode.Position) => {
            const idx = typeof i === 'number' ? i : i.line;
            return { text: lines[idx] };
        },
        getText: (range?: vscode.Range) => {
            if (!range) { return lines.join('\n'); }
            return lines.slice(range.start.line, range.end.line + 1).join('\n');
        },
        getWordRangeAtPosition: jest.fn().mockReturnValue(undefined)
    };
}

function pos(line: number, character: number) {
    return new vscode.Position(line, character);
}

let provideHover: (doc: ReturnType<typeof makeDocument>, position: vscode.Position) => Promise<vscode.Hover | undefined>;

beforeAll(() => {
    const calls = (vscode.languages.registerHoverProvider as jest.Mock).mock.calls;
    provideHover = calls[calls.length - 1][1].provideHover;
});

beforeEach(() => {
    jest.clearAllMocks();
    setEnvironmentProvider({ getSelectedEnvironment: () => undefined });
});

describe('rq declaration hover', () => {
    test('shows method and URL when cursor is on rq keyword', async () => {
        const doc = makeDocument(['rq get("https://api.example.com/users");']);
        const result = await provideHover(doc, pos(0, 1)) as vscode.Hover;
        expect(result).toBeInstanceOf(vscode.Hover);
        expect((result.contents as unknown as vscode.MarkdownString).value).toContain('GET');
        expect((result.contents as unknown as vscode.MarkdownString).value).toContain('https://api.example.com/users');
    });

    test('shows method and URL when cursor is on request name', async () => {
        const doc = makeDocument(['rq get("https://api.example.com/users");']);
        const result = await provideHover(doc, pos(0, 5)) as vscode.Hover;
        expect(result).toBeInstanceOf(vscode.Hover);
        expect((result.contents as unknown as vscode.MarkdownString).value).toContain('GET');
    });

    test('infers method from request name', async () => {
        const doc = makeDocument(['rq post("https://api.example.com/users");']);
        const result = await provideHover(doc, pos(0, 1)) as vscode.Hover;
        expect((result!.contents as unknown as vscode.MarkdownString).value).toContain('POST');
    });

    test('resolves method from [method(...)] attribute on preceding line', async () => {
        const doc = makeDocument([
            '[method(PUT)]',
            'rq update("https://api.example.com/users/1");',
        ]);
        const result = await provideHover(doc, pos(1, 1)) as vscode.Hover;
        expect((result!.contents as unknown as vscode.MarkdownString).value).toContain('PUT');
    });

    test('defaults to GET when name is not an HTTP verb and no attribute', async () => {
        const doc = makeDocument(['rq fetch_data("https://api.example.com");']);
        const result = await provideHover(doc, pos(0, 1)) as vscode.Hover;
        expect((result!.contents as unknown as vscode.MarkdownString).value).toContain('GET');
    });

    test('describes the qs property inside a templated ep declaration', async () => {
        const lines = ['ep widgets<base>("/widgets", qs: "v=1") {'];
        const column = lines[0].indexOf('qs');
        const doc = {
            ...makeDocument(lines),
            getText: (range?: vscode.Range) => {
                if (!range) { return lines.join('\n'); }
                if (range.start.line === range.end.line) {
                    return lines[range.start.line].slice(range.start.character, range.end.character);
                }
                return lines.slice(range.start.line, range.end.line + 1).join('\n');
            },
            getWordRangeAtPosition: jest.fn().mockImplementation((_p: vscode.Position, re: RegExp) =>
                re.test('qs') ? new vscode.Range(pos(0, column), pos(0, column + 2)) : undefined
            )
        };

        const result = await provideHover(doc, pos(0, column)) as vscode.Hover;

        expect(result).toBeInstanceOf(vscode.Hover);
        expect((result.contents as unknown as vscode.MarkdownString).value).toContain('Endpoint Property');
        expect((result.contents as unknown as vscode.MarkdownString).value).toContain('qs');
    });

    test('returns undefined when cursor is past the opening paren', async () => {
        const doc = makeDocument(['rq get("https://api.example.com");']);
        const result = await provideHover(doc, pos(0, 20));
        expect(result).toBeUndefined();
    });

    test('matches hyphenated request names', async () => {
        const doc = makeDocument(['rq get-users("https://api.example.com/users");']);
        const result = await provideHover(doc, pos(0, 1)) as vscode.Hover;
        expect(result).toBeInstanceOf(vscode.Hover);
        expect((result.contents as unknown as vscode.MarkdownString).value).toContain('get-users');
    });

    test('does not trigger variable lookup for request name', async () => {
        setEnvironmentProvider({ getSelectedEnvironment: () => 'local' });
        const doc = makeDocument(['rq get_users("https://api.example.com");']);
        await provideHover(doc, pos(0, 4));
        expect(cliService.showVariable).not.toHaveBeenCalled();
    });
});

describe('rq declaration hover — parameter signature', () => {
    const valueOf = (result: vscode.Hover) => (result.contents as unknown as vscode.MarkdownString).value;

    test('lists every parameter, marking the ones not provided', async () => {
        const doc = makeDocument(['rq plain("http://localhost:8080/get");']);
        const result = await provideHover(doc, pos(0, 1)) as vscode.Hover;
        expect(valueOf(result)).toContain('rq plain(');
        expect(valueOf(result)).toContain('url:     "http://localhost:8080/get",');
        expect(valueOf(result)).toContain('headers: <not set>,');
        expect(valueOf(result)).toContain('body:    <not set>,');
    });

    test('reads parameters from a multiline declaration', async () => {
        const doc = makeDocument([
            'rq create(',
            '    "/users",',
            '    body: ${"name": "{{user_name}}"}',
            ');',
        ]);
        const result = await provideHover(doc, pos(0, 1)) as vscode.Hover;
        expect(valueOf(result)).toContain('url:     "/users",');
        expect(valueOf(result)).toContain('body:    ${"name": "{{user_name}}"},');
        expect(valueOf(result)).toContain('headers: <not set>,');
    });

    test('assigns named parameters regardless of order', async () => {
        const doc = makeDocument(['rq mixed(body: "b", url: "u");']);
        const result = await provideHover(doc, pos(0, 1)) as vscode.Hover;
        expect(valueOf(result)).toContain('url:     "u",');
        expect(valueOf(result)).toContain('body:    "b",');
    });

    test('does not mistake a comma inside a headers literal for a separator', async () => {
        const doc = makeDocument(['rq h("u", $["A": "1", "B": "2"]);']);
        const result = await provideHover(doc, pos(0, 1)) as vscode.Hover;
        expect(valueOf(result)).toContain('headers: $["A": "1", "B": "2"],');
        expect(valueOf(result)).toContain('body:    <not set>,');
    });

    test('reports timeout, auth and required attributes', async () => {
        const doc = makeDocument([
            '[method(POST)]',
            '[timeout(10)]',
            '[auth("my_auth")]',
            '[required(user_name)]',
            'rq create("/users");',
        ]);
        const result = await provideHover(doc, pos(4, 1)) as vscode.Hover;
        expect(valueOf(result)).toContain('**Method:** `POST`');
        expect(valueOf(result)).toContain('**Timeout:** `10s`');
        expect(valueOf(result)).toContain('**Auth:** `my_auth`');
        expect(valueOf(result)).toContain('**Required:** `user_name`');
    });

    test('truncates a very long parameter value', async () => {
        const doc = makeDocument([`rq long("${'x'.repeat(120)}");`]);
        const result = await provideHover(doc, pos(0, 1)) as vscode.Hover;
        expect(valueOf(result)).toContain('…');
    });
});

describe('ep declaration hover', () => {
    test('shows endpoint name and base URL', async () => {
        const doc = makeDocument(['ep users("https://api.example.com/users") {']);
        const result = await provideHover(doc, pos(0, 1)) as vscode.Hover;
        expect(result).toBeInstanceOf(vscode.Hover);
        const value = (result.contents as unknown as vscode.MarkdownString).value;
        expect(value).toContain('users');
        expect(value).toContain('https://api.example.com/users');
    });

    test('shows extends info when template syntax is used', async () => {
        const doc = makeDocument(['ep users<base>("https://api.example.com/users") {']);
        const result = await provideHover(doc, pos(0, 1)) as vscode.Hover;
        const value = (result!.contents as unknown as vscode.MarkdownString).value;
        expect(value).toContain('base');
    });

    test('lists url, headers and qs, marking the ones not provided', async () => {
        const doc = makeDocument(['ep widgets<base>("/widgets", qs: "v=1") {']);
        const result = await provideHover(doc, pos(0, 1)) as vscode.Hover;
        const value = (result!.contents as unknown as vscode.MarkdownString).value;
        expect(value).toContain('ep widgets<base>(');
        expect(value).toContain('url:     "/widgets",');
        expect(value).toContain('headers: <not set>,');
        expect(value).toContain('qs:      "v=1",');
    });

    test('marks every parameter as not set for a body-only endpoint', async () => {
        const doc = makeDocument(['ep users {', '    rq list();', '}']);
        const result = await provideHover(doc, pos(0, 1)) as vscode.Hover;
        const value = (result!.contents as unknown as vscode.MarkdownString).value;
        expect(value).toContain('url:     <not set>,');
        expect(value).toContain('qs:      <not set>,');
    });

    test('reports timeout and auth attributes', async () => {
        const doc = makeDocument(['[timeout(20)]', '[auth("svc")]', 'ep users("/users") {']);
        const result = await provideHover(doc, pos(2, 1)) as vscode.Hover;
        const value = (result!.contents as unknown as vscode.MarkdownString).value;
        expect(value).toContain('**Timeout:** `20s`');
        expect(value).toContain('**Auth:** `svc`');
    });

    test('returns undefined when cursor is past the opening paren', async () => {
        const doc = makeDocument(['ep users("https://api.example.com") {']);
        const result = await provideHover(doc, pos(0, 30));
        expect(result).toBeUndefined();
    });
});

describe('auth declaration hover', () => {
    test('shows auth name and formatted type for bearer', async () => {
        const doc = makeDocument(['auth my_token(auth_type.bearer) {']);
        const result = await provideHover(doc, pos(0, 1)) as vscode.Hover;
        expect(result).toBeInstanceOf(vscode.Hover);
        const value = (result.contents as unknown as vscode.MarkdownString).value;
        expect(value).toContain('my_token');
        expect(value).toContain('Bearer Token');
    });

    test('shows formatted type for oauth2_client_credentials', async () => {
        const doc = makeDocument(['auth svc(auth_type.oauth2_client_credentials) {']);
        const result = await provideHover(doc, pos(0, 1)) as vscode.Hover;
        const value = (result!.contents as unknown as vscode.MarkdownString).value;
        expect(value).toContain('OAuth2 Client Credentials');
    });

    test('lists every field of the auth type, marking the ones not defined', async () => {
        const doc = makeDocument([
            'auth svc(auth_type.oauth2_client_credentials) {',
            '    client_id: "{{id}}",',
            '    scope: "read",',
            '}',
        ]);
        const result = await provideHover(doc, pos(0, 1)) as vscode.Hover;
        const value = (result!.contents as unknown as vscode.MarkdownString).value;
        expect(value).toContain('client_id:     "{{id}}",');
        expect(value).toContain('token_url:     <not set>,');
        expect(value).toContain('cert_file:     <not set>,');
        expect(value).toContain('scope:         "read",');
        expect(value).toContain('**Required fields:** `client_id`, `token_url`');
        expect(value).toContain('**Missing:** `token_url`');
    });

    test('does not report missing fields when every required one is defined', async () => {
        const doc = makeDocument(['auth t(auth_type.bearer) {', '    token: "{{api_key}}",', '}']);
        const result = await provideHover(doc, pos(0, 1)) as vscode.Hover;
        const value = (result!.contents as unknown as vscode.MarkdownString).value;
        expect(value).toContain('token: "{{api_key}}",');
        expect(value).not.toContain('**Missing:**');
    });

    test('returns undefined when cursor is past the opening paren', async () => {
        const doc = makeDocument(['auth my_token(auth_type.bearer) {']);
        const result = await provideHover(doc, pos(0, 20));
        expect(result).toBeUndefined();
    });
});

describe('env declaration hover', () => {
    test('shows environment name', async () => {
        const doc = makeDocument(['env local {', '  base_url: "http://localhost",', '}']);
        const result = await provideHover(doc, pos(0, 1)) as vscode.Hover;
        expect(result).toBeInstanceOf(vscode.Hover);
        expect((result.contents as unknown as vscode.MarkdownString).value).toContain('local');
    });

    test('lists variables from env block', async () => {
        const doc = makeDocument([
            'env local {',
            '  base_url: "http://localhost",',
            '  api_key: "dev-key",',
            '}',
        ]);
        const result = await provideHover(doc, pos(0, 1)) as vscode.Hover;
        const value = (result!.contents as unknown as vscode.MarkdownString).value;
        expect(value).toContain('base_url');
        expect(value).toContain('api_key');
    });

    test('lists env variables with their values', async () => {
        const doc = makeDocument([
            'env local {',
            '  base_url: "http://localhost:8080",',
            '  api_key: "xyz",',
            '}',
        ]);
        const result = await provideHover(doc, pos(0, 1)) as vscode.Hover;
        const value = (result!.contents as unknown as vscode.MarkdownString).value;
        expect(value).toContain('env local {');
        expect(value).toContain('base_url: "http://localhost:8080",');
        expect(value).toContain('api_key:  "xyz",');
    });

    test('shows overflow count when env has more variables than fit', async () => {
        const entries = Array.from({ length: 14 }, (_, i) => `  v${i}: "${i}",`);
        const doc = makeDocument(['env large {', ...entries, '}']);
        const result = await provideHover(doc, pos(0, 1)) as vscode.Hover;
        const value = (result!.contents as unknown as vscode.MarkdownString).value;
        expect(value).toContain('+2 more');
    });

    test('does not miscount braces inside string values', async () => {
        const doc = makeDocument([
            'env local {',
            '  body: ${"nested": {"key": "val"}},',
            '  api_key: "dev",',
            '}',
        ]);
        const result = await provideHover(doc, pos(0, 1)) as vscode.Hover;
        const value = (result!.contents as unknown as vscode.MarkdownString).value;
        expect(value).toContain('api_key');
    });

    test('returns undefined when cursor is past the env name', async () => {
        const doc = makeDocument(['env local {']);
        const result = await provideHover(doc, pos(0, 11));
        expect(result).toBeUndefined();
    });
});

describe('import hover', () => {
    test('describes what an import pulls in and resolves the file name', async () => {
        const doc = makeDocument(['import "base";']);
        const result = await provideHover(doc, pos(0, 2)) as vscode.Hover;
        const value = (result!.contents as unknown as vscode.MarkdownString).value;
        expect(value).toContain('base.rq');
        expect(value).toContain('import "base";');
    });

    test('keeps an explicit .rq extension', async () => {
        const doc = makeDocument(['import "shared.rq";']);
        const result = await provideHover(doc, pos(0, 2)) as vscode.Hover;
        expect((result!.contents as unknown as vscode.MarkdownString).value).toContain('shared.rq');
    });
});

describe('attribute hover', () => {
    test('describes [method] with its allowed verbs', async () => {
        const doc = makeDocument(['[method(POST)]', 'rq create("/users");']);
        const result = await provideHover(doc, pos(0, 2)) as vscode.Hover;
        const value = (result!.contents as unknown as vscode.MarkdownString).value;
        expect(value).toContain('**Attribute: `method`**');
        expect(value).toContain('[method(VERB)]');
        expect(value).toContain('**Applies to:** `rq`');
        expect(value).toContain('PATCH');
    });

    test('describes [timeout] as valid on both rq and ep', async () => {
        const doc = makeDocument(['[timeout(10)]', 'rq slow("/x");']);
        const result = await provideHover(doc, pos(0, 2)) as vscode.Hover;
        expect((result!.contents as unknown as vscode.MarkdownString).value).toContain('**Applies to:** `rq, ep`');
    });

    test('ignores an unknown attribute name', async () => {
        const doc = makeDocument(['[unknown(1)]', 'rq x("/x");']);
        const result = await provideHover(doc, pos(0, 2));
        expect(result).toBeUndefined();
    });
});

describe('local variable hover', () => {
    function documentWithWordAt(lines: string[], line: number, word: string) {
        const column = lines[line].indexOf(word);
        return {
            lineCount: lines.length,
            lineAt: (i: number | vscode.Position) => {
                const idx = typeof i === 'number' ? i : i.line;
                return { text: lines[idx] };
            },
            getText: (range?: vscode.Range) => {
                if (!range) { return lines.join('\n'); }
                if (range.start.line === range.end.line) {
                    return lines[range.start.line].slice(range.start.character, range.end.character);
                }
                return lines.slice(range.start.line, range.end.line + 1).join('\n');
            },
            getWordRangeAtPosition: jest.fn().mockImplementation((_p: vscode.Position, re: RegExp) =>
                re.test(word) ? new vscode.Range(pos(line, column), pos(line, column + word.length)) : undefined
            )
        };
    }

    test('renders the declaration with a single statement terminator', async () => {
        const lines = ['let token = "abc";', 'let header = "Bearer {{token}}";'];
        const doc = documentWithWordAt(lines, 1, 'token');
        const result = await provideHover(doc, pos(1, lines[1].indexOf('token'))) as vscode.Hover;
        const value = (result!.contents as unknown as vscode.MarkdownString).value;
        expect(value).toContain('**Variable: `token`**');
        expect(value).toContain('let token = "abc";');
        expect(value).not.toContain(';;');
    });

    test('reports the line the variable was declared on', async () => {
        const lines = ['let a = "1";', 'let token = "abc";', 'let header = "{{token}}";'];
        const doc = documentWithWordAt(lines, 2, 'token');
        const result = await provideHover(doc, pos(2, lines[2].indexOf('token'))) as vscode.Hover;
        expect((result!.contents as unknown as vscode.MarkdownString).value).toContain('Defined on line 2');
    });
});
