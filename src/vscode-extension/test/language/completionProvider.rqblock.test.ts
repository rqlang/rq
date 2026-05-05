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
