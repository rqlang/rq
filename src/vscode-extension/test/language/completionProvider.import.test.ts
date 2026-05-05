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

describe('import completion', () => {
    test('suggests workspace .rq files without extension when typing "import "', async () => {
        (vscode.workspace.findFiles as jest.Mock).mockResolvedValue([
            { fsPath: '/workspace/shared.rq' },
            { fsPath: '/workspace/auth.rq' }
        ]);

        const doc = makeDocument(['import '], { fsPath: '/workspace/current.rq' });
        const position = new vscode.Position(0, 7);

        const items = await provideCompletionItems(doc, position);

        expect(items).toHaveLength(2);
        expect(items[0].label).toBe('shared');
        expect(items[0].insertText).toBe('"shared";');
        expect(items[1].label).toBe('auth');
        expect(items[1].insertText).toBe('"auth";');
    });

    test('excludes current file from suggestions', async () => {
        (vscode.workspace.findFiles as jest.Mock).mockResolvedValue([
            { fsPath: '/workspace/current.rq' },
            { fsPath: '/workspace/other.rq' }
        ]);

        const doc = makeDocument(['import '], { fsPath: '/workspace/current.rq' });
        const position = new vscode.Position(0, 7);

        const items = await provideCompletionItems(doc, position);

        expect(items).toHaveLength(1);
        expect(items[0].label).toBe('other');
    });

    test('uses relative path for files in subdirectories', async () => {
        (vscode.workspace.findFiles as jest.Mock).mockResolvedValue([
            { fsPath: '/workspace/sub/nested.rq' }
        ]);

        const doc = makeDocument(['import '], { fsPath: '/workspace/current.rq' });
        const position = new vscode.Position(0, 7);

        const items = await provideCompletionItems(doc, position);

        expect(items).toHaveLength(1);
        expect(items[0].label).toBe('sub/nested');
        expect(items[0].insertText).toBe('"sub/nested";');
    });

    test('inserts with leading space when "import" typed without trailing space', async () => {
        (vscode.workspace.findFiles as jest.Mock).mockResolvedValue([
            { fsPath: '/workspace/shared.rq' }
        ]);

        const doc = makeDocument(['import'], { fsPath: '/workspace/current.rq' });
        const position = new vscode.Position(0, 6);

        const items = await provideCompletionItems(doc, position);

        expect(items).toHaveLength(1);
        expect(items[0].insertText).toBe(' "shared";');
    });

    test('returns empty list when no other files exist', async () => {
        (vscode.workspace.findFiles as jest.Mock).mockResolvedValue([]);

        const doc = makeDocument(['import '], { fsPath: '/workspace/current.rq' });
        const position = new vscode.Position(0, 7);

        const items = await provideCompletionItems(doc, position);

        expect(items).toHaveLength(0);
    });

    test('returns empty list when only current file exists', async () => {
        (vscode.workspace.findFiles as jest.Mock).mockResolvedValue([
            { fsPath: '/workspace/current.rq' }
        ]);

        const doc = makeDocument(['import '], { fsPath: '/workspace/current.rq' });
        const position = new vscode.Position(0, 7);

        const items = await provideCompletionItems(doc, position);

        expect(items).toHaveLength(0);
    });

    test('hides import file completions when typing "import" after let', async () => {
        (vscode.workspace.findFiles as jest.Mock).mockResolvedValue([
            { fsPath: '/workspace/shared.rq' }
        ]);
        const lines = ['let base = "http://localhost";', 'import '];
        const doc = makeDocument(lines, { fsPath: '/workspace/current.rq' });
        const position = new vscode.Position(1, 7);

        const items = await provideCompletionItems(doc, position);

        expect(items).toBeUndefined();
    });
});
