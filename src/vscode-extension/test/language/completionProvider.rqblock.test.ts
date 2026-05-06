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
    test('does not suggest params on bare comma inside rq()', async () => {
        const lines = ['rq my_rq(', '    "url",'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        expect(items === undefined || !items.some((i: any) => i.label === 'headers')).toBe(true);
    });

    test('suggests params after comma + space inside rq()', async () => {
        const lines = ['rq my_rq(', '    "url", '];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

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
    test('does not suggest params on bare comma inside ep()', async () => {
        const lines = ['ep my_ep(', '    "url",'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        expect(items === undefined || !items.some((i: any) => i.label === 'headers')).toBe(true);
    });

    test('suggests params after comma + space inside ep()', async () => {
        const lines = ['ep my_ep(', '    "url", '];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        expect(items).not.toBeUndefined();
        expect(items.some((i: any) => i.label === 'headers')).toBe(true);
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
