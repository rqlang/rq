jest.mock('../../src/rqClient');
jest.mock('../../src/utils', () => ({
    ...jest.requireActual('../../src/utils'),
    mirrorToTemp: jest.fn().mockReturnValue('/tmp/rq-check-mock'),
}));

import * as vscode from 'vscode';
import * as cliService from '../../src/rqClient';
import * as utils from '../../src/utils';
import '../../src/language/completionProvider';
import { makeDocument } from './completionTestUtils';

let provideCompletionItems: Function;

beforeAll(() => {
    const calls = (vscode.languages.registerCompletionItemProvider as jest.Mock).mock.calls;
    provideCompletionItems = calls[calls.length - 1][1].provideCompletionItems;
});

beforeEach(() => {
    jest.clearAllMocks();
    (vscode.workspace.findFiles as jest.Mock).mockResolvedValue([]);
    (vscode.workspace.textDocuments as any) = [];
    (vscode.workspace.getWorkspaceFolder as jest.Mock).mockReturnValue(undefined);
    (cliService.listEndpoints as jest.Mock).mockResolvedValue([]);
    (cliService.listVariables as jest.Mock).mockResolvedValue([]);
    (cliService.listAuthConfigs as jest.Mock).mockResolvedValue([]);
    (utils.mirrorToTemp as jest.Mock).mockReturnValue('/tmp/rq-check-mock');
});

describe('rq block param completion — comma trigger', () => {
    test('does not suggest params on bare comma typed inside rq()', async () => {
        const lines = ['rq my_rq(', '    "url",'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);
        const context = { triggerKind: vscode.CompletionTriggerKind.TriggerCharacter };

        const items = await provideCompletionItems(doc, position, undefined, context);

        expect(items === undefined || !items.some((i: any) => i.label === 'headers')).toBe(true);
    });

    test('suggests params on bare comma when explicitly invoked inside rq()', async () => {
        const lines = ['rq my_rq(', '    "url",'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);
        const context = { triggerKind: vscode.CompletionTriggerKind.Invoke };

        const items = await provideCompletionItems(doc, position, undefined, context);

        expect(items).not.toBeUndefined();
        expect(items.some((i: any) => i.label === 'headers')).toBe(true);
    });

    test('suggests params inside rq() declared with a hyphenated name', async () => {
        const lines = ['rq my-rq("/users", '];
        const doc = makeDocument(lines);
        const position = new vscode.Position(0, lines[0].length);

        const items = await provideCompletionItems(doc, position);

        expect(items).not.toBeUndefined();
        expect(items.some((i: any) => i.label === 'headers')).toBe(true);
    });

    test('suggests params after comma + space inside rq()', async () => {
        const lines = ['rq my_rq(', '    "url", '];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        expect(items).not.toBeUndefined();
        expect(items.some((i: any) => i.label === 'headers')).toBe(true);
    });

    test('does not suggest params on a new line when the previous argument has no comma', async () => {
        const lines = ['rq my_rq(', '    "url"', '    '];
        const doc = makeDocument(lines);
        const position = new vscode.Position(2, lines[2].length);

        const items = await provideCompletionItems(doc, position);

        expect(items === undefined || !items.some((i: any) => i.label === 'headers')).toBe(true);
    });

    test('suggests params on a new line after a comma', async () => {
        const lines = ['rq my_rq(', '    "url",', '    '];
        const doc = makeDocument(lines);
        const position = new vscode.Position(2, lines[2].length);

        const items = await provideCompletionItems(doc, position);

        expect(items).not.toBeUndefined();
        expect(items.some((i: any) => i.label === 'headers')).toBe(true);
    });

    test('does not suggest params inside ${ } json literal in rq()', async () => {
        const lines = [
            'rq my_rq(',
            '    "url",',
            '    $[',
            '        "Accept": "pepito"',
            '    ],',
            '    ${',
            '        ',
        ];
        const doc = makeDocument(lines);
        const position = new vscode.Position(6, lines[6].length);

        const items = await provideCompletionItems(doc, position);

        expect(items === undefined || !items.some((i: any) => i.label === 'headers')).toBe(true);
    });
});

describe('ep block param completion — comma trigger', () => {
    test('does not suggest params on bare comma typed inside ep()', async () => {
        const lines = ['ep my_ep(', '    "url",'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);
        const context = { triggerKind: vscode.CompletionTriggerKind.TriggerCharacter };

        const items = await provideCompletionItems(doc, position, undefined, context);

        expect(items === undefined || !items.some((i: any) => i.label === 'headers')).toBe(true);
    });

    test('suggests params on bare comma when explicitly invoked inside ep()', async () => {
        const lines = ['ep my_ep("/users",'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(0, lines[0].length);
        const context = { triggerKind: vscode.CompletionTriggerKind.Invoke };

        const items = await provideCompletionItems(doc, position, undefined, context);

        expect(items).not.toBeUndefined();
        expect(items.some((i: any) => i.label === 'headers')).toBe(true);
        expect(items.some((i: any) => i.label === 'qs')).toBe(true);
    });

    test('suggests params inside a templated ep() on comma + space', async () => {
        const lines = ['ep widgets<base>("/widgets", ) {', '    rq list();', '}'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(0, 'ep widgets<base>("/widgets", '.length);

        const items = await provideCompletionItems(doc, position);

        expect(items).not.toBeUndefined();
        expect(items.some((i: any) => i.label === 'qs')).toBe(true);
    });

    test('suggests params inside a templated ep() on bare comma when explicitly invoked', async () => {
        const lines = ['ep widgets<base>("/widgets",) {', '    rq list();', '}'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(0, 'ep widgets<base>("/widgets",'.length);
        const context = { triggerKind: vscode.CompletionTriggerKind.Invoke };

        const items = await provideCompletionItems(doc, position, undefined, context);

        expect(items).not.toBeUndefined();
        expect(items.some((i: any) => i.label === 'qs')).toBe(true);
    });

    test('suggests url at the start of a templated ep() param list', async () => {
        const lines = ['ep widgets < base > ('];
        const doc = makeDocument(lines);
        const position = new vscode.Position(0, lines[0].length);

        const items = await provideCompletionItems(doc, position);

        expect(items).not.toBeUndefined();
        expect(items.some((i: any) => i.label === 'url')).toBe(true);
    });

    test('suggests params inside ep() declared with a hyphenated name', async () => {
        const lines = ['ep user-api("/users", ) {', '    rq list("?v=1");', '}'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(0, 'ep user-api("/users", '.length);

        const items = await provideCompletionItems(doc, position);

        expect(items).not.toBeUndefined();
        expect(items.some((i: any) => i.label === 'qs')).toBe(true);
    });

    test('suggests params after comma + space inside ep()', async () => {
        const lines = ['ep my_ep(', '    "url", '];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        expect(items).not.toBeUndefined();
        expect(items.some((i: any) => i.label === 'headers')).toBe(true);
    });

    test('does not suggest params on a new line when the previous argument has no comma', async () => {
        const lines = ['[auth("my_auth2")]', 'ep base(url: "http://localhost:8080"', '    ', ');'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(2, lines[2].length);

        const items = await provideCompletionItems(doc, position);

        expect(items === undefined || !items.some((i: any) => i.label === 'qs')).toBe(true);
    });

    test('suggests params on a new line after a comma', async () => {
        const lines = ['[auth("my_auth2")]', 'ep base(url: "http://localhost:8080",', '    ', ');'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(2, lines[2].length);

        const items = await provideCompletionItems(doc, position);

        expect(items).not.toBeUndefined();
        expect(items.some((i: any) => i.label === 'qs')).toBe(true);
    });

    test('does not suggest a partial param on a new line when the previous argument has no comma', async () => {
        const lines = ['ep base(url: "http://localhost:8080"', '    he', ');'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        expect(items === undefined || !items.some((i: any) => i.label === 'headers')).toBe(true);
    });

    test('suggests params on a new line right after the opening paren', async () => {
        const lines = ['ep base(', '    '];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        expect(items).not.toBeUndefined();
        expect(items.some((i: any) => i.label === 'url')).toBe(true);
    });

    test('does not suggest params inside ${ } json literal in ep()', async () => {
        const lines = [
            'ep my_ep(',
            '    "url",',
            '    ${',
            '        ',
        ];
        const doc = makeDocument(lines);
        const position = new vscode.Position(3, lines[3].length);

        const items = await provideCompletionItems(doc, position);

        expect(items === undefined || !items.some((i: any) => i.label === 'headers')).toBe(true);
    });
});

describe('rq block param completion — partial word typed', () => {
    test('suggests params when partial word typed after ( on same line', async () => {
        const lines = ['rq my_rq(ur'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(0, lines[0].length);

        const items = await provideCompletionItems(doc, position);

        expect(items).toBeDefined();
        expect(items.some((i: any) => i.label === 'url')).toBe(true);
        const target = items.find((i: any) => i.label === 'url');
        expect(target.range.start.character).toBe(9);
        expect(target.range.end.character).toBe(11);
    });

    test('suggests variables when partial word typed after named param colon', async () => {
        (cliService.listVariables as jest.Mock).mockResolvedValue([
            { name: 'base_url', value: 'http://localhost', file: '/workspace/shared.rq', line: 0, character: 0, source: 'let' }
        ]);

        const lines = ['rq my_rq(', '    url: bas'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        expect(items).toBeDefined();
        const target = items.find((i: any) => i.label === 'base_url');
        expect(target).toBeDefined();
        expect(target.range.start.character).toBe(9);
        expect(target.range.end.character).toBe(12);
    });

    test('suggests params when partial word typed after comma on same line', async () => {
        const doc = makeDocument(['rq my_rq("https://api.example.com", hea']);
        const position = new vscode.Position(0, 39);

        const items = await provideCompletionItems(doc, position);

        expect(items).toBeDefined();
        expect(items.some((i: any) => i.label === 'headers')).toBe(true);
        const target = items.find((i: any) => i.label === 'headers');
        expect(target.range.start.character).toBe(36);
        expect(target.range.end.character).toBe(39);
    });
});

describe('rq block param completion — slots already claimed', () => {
    test('does not suggest headers when a positional headers array follows a named url', async () => {
        const doc = makeDocument(['rq my_rq(url: "http://x", $["A": "1"], ']);
        const position = new vscode.Position(0, 39);

        const items = await provideCompletionItems(doc, position);

        expect(items).toBeDefined();
        expect(items.some((i: any) => i.label === 'headers')).toBe(false);
        expect(items.some((i: any) => i.label === 'body')).toBe(true);
    });

    test('does not treat a header key as a named argument', async () => {
        const doc = makeDocument(['rq my_rq(url: "http://x", $["body": "1"], ']);
        const position = new vscode.Position(0, 42);

        const items = await provideCompletionItems(doc, position);

        expect(items).toBeDefined();
        expect(items.some((i: any) => i.label === 'body')).toBe(true);
    });

    test('does not suggest qs when an ep has claimed every slot', async () => {
        const doc = makeDocument(['ep base(headers: $["A": "1"], "http://x", "v=1", ']);
        const position = new vscode.Position(0, 48);

        const items = await provideCompletionItems(doc, position);

        expect(items === undefined || !items.some((i: any) => i.label === 'qs')).toBe(true);
    });
});
