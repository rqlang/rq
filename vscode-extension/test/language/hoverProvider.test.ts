jest.mock('../../src/cliService');

import * as vscode from 'vscode';
import * as cliService from '../../src/cliService';
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

    test('returns undefined when cursor is past the opening paren', async () => {
        const doc = makeDocument(['rq get("https://api.example.com");']);
        const result = await provideHover(doc, pos(0, 20));
        expect(result).toBeUndefined();
    });

    test('does not trigger variable lookup for request name', async () => {
        setEnvironmentProvider({ getSelectedEnvironment: () => 'local' });
        const doc = makeDocument(['rq get_users("https://api.example.com");']);
        await provideHover(doc, pos(0, 4));
        expect(cliService.showVariable).not.toHaveBeenCalled();
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

    test('shows overflow count when env has more than 5 variables', async () => {
        const doc = makeDocument([
            'env large {',
            '  a: "1",', '  b: "2",', '  c: "3",', '  d: "4",', '  e: "5",', '  f: "6",',
            '}',
        ]);
        const result = await provideHover(doc, pos(0, 1)) as vscode.Hover;
        const value = (result!.contents as unknown as vscode.MarkdownString).value;
        expect(value).toContain('+1 more');
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
