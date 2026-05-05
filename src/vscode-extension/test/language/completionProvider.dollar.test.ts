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

describe('dollar prefix completion', () => {
    test('suggests ${ } and $[ ] in standalone context', async () => {
        const doc = makeDocument(['let x = $']);
        const position = new vscode.Position(0, 9);

        const items = await provideCompletionItems(doc, position);

        const json = items.find((i: any) => i.label === '${ }');
        expect(json).toBeDefined();
        expect(json.insertText.value).toContain('\n');
        expect(json.insertText.value).toContain(';');

        const headers = items.find((i: any) => i.label === '$[ ]');
        expect(headers).toBeDefined();
        expect(headers.insertText.value).toContain('\n');
        expect(headers.insertText.value).toContain(';');
    });

    test('suggests compact ${ } without newlines or semicolon inside rq block', async () => {
        const lines = ['rq foo(body: $'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(0, lines[0].length);

        const items = await provideCompletionItems(doc, position);

        const json = items.find((i: any) => i.label === '${ }');
        expect(json).toBeDefined();
        expect(json.insertText.value).not.toContain('\n');
        expect(json.insertText.value).not.toContain(';');
    });

    test('suggests compact $[ ] without newlines or semicolon inside rq block', async () => {
        const lines = ['rq foo(headers: $'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(0, lines[0].length);

        const items = await provideCompletionItems(doc, position);

        const headers = items.find((i: any) => i.label === '$[ ]');
        expect(headers).toBeDefined();
        expect(headers.insertText.value).not.toContain('\n');
        expect(headers.insertText.value).not.toContain(';');
    });

    test('suggests compact completions inside ep block rq call', async () => {
        const lines = ['ep api("http://api") {', '    rq get(headers: $'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        const headers = items.find((i: any) => i.label === '$[ ]');
        expect(headers).toBeDefined();
        expect(headers.insertText.value).not.toContain(';');

        const json = items.find((i: any) => i.label === '${ }');
        expect(json).toBeDefined();
        expect(json.insertText.value).not.toContain(';');
    });
});
