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

describe('header key completion', () => {
    test('does not suggest headers on same line immediately after [', async () => {
        const lines = ['let my_h = ['];
        const doc = makeDocument(lines);
        const position = new vscode.Position(0, lines[0].length);

        const items = await provideCompletionItems(doc, position);

        expect(items === undefined || !items.some((i: any) => i.label === 'Content-Type')).toBe(true);
    });

    test('does not suggest headers on same line after opening quote in [', async () => {
        const lines = ['let my_h = ["'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(0, lines[0].length);

        const items = await provideCompletionItems(doc, position);

        expect(items === undefined || !items.some((i: any) => i.label === 'Authorization')).toBe(true);
    });

    test('suggests headers on blank new line inside array (after pressing Enter)', async () => {
        const lines = ['    "Content-Type": "application/json",', '    '];
        const doc = makeDocument(lines, { fsPath: '/workspace/current.rq' });
        (doc as any).getText = jest.fn().mockReturnValue('let my_h = $[\n' + lines[0] + '\n' + lines[1]);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        expect(items).not.toBeUndefined();
        const auth = items.find((i: any) => i.label === 'Authorization');
        expect(auth).toBeDefined();
        expect(auth.insertText.value).toBe('"Authorization": "${1:}"');
    });

    test('does not suggest headers outside array literal', async () => {
        const lines = ['"'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(0, lines[0].length);

        const items = await provideCompletionItems(doc, position);

        expect(items === undefined || !items.some((i: any) => i.label === 'Content-Type')).toBe(true);
    });

    test('falls back to local variables when CLI is unavailable', async () => {
        (cliService.listVariables as jest.Mock).mockRejectedValue(new Error('CLI error'));

        const lines = ['let base_url = "http://localhost"', 'env dev {', '    api_url: "'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(2, lines[2].length);

        const items = await provideCompletionItems(doc, position);

        expect(items.find((i: any) => i.label === 'base_url')?.insertText).toBe('base_url');
    });
});
