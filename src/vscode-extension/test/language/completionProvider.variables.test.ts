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

describe('variable reference completion', () => {
    test('suggests variables when typing "let name = "', async () => {
        (cliService.listVariables as jest.Mock).mockResolvedValue([
            { name: 'base_url', value: 'http://localhost', file: '/workspace/shared.rq', line: 0, character: 0, source: 'let' },
            { name: 'token', value: 'abc123', file: '/workspace/shared.rq', line: 1, character: 0, source: 'let' }
        ]);

        const doc = makeDocument(['let my_var = ']);
        const position = new vscode.Position(0, 13);

        const items = await provideCompletionItems(doc, position);

        expect(cliService.listVariables).toHaveBeenCalledWith('/workspace/current.rq', undefined);
        expect(items.find((i: any) => i.label === 'base_url')?.insertText).toBe('base_url;');
        expect(items.find((i: any) => i.label === 'base_url')?.detail).toBe('= http://localhost');
        expect(items.find((i: any) => i.label === 'token')).toBeDefined();
    });

    test('uses source as detail when value is empty', async () => {
        (cliService.listVariables as jest.Mock).mockResolvedValue([
            { name: 'my_var', value: '', file: '/workspace/shared.rq', line: 0, character: 0, source: 'let' }
        ]);

        const doc = makeDocument(['let result = ']);
        const position = new vscode.Position(0, 13);

        const items = await provideCompletionItems(doc, position);

        expect(items.find((i: any) => i.label === 'my_var')?.detail).toBe('let');
    });

    test('returns builtin functions even when no variables exist', async () => {
        (cliService.listVariables as jest.Mock).mockResolvedValue([]);

        const doc = makeDocument(['let a = ']);
        const position = new vscode.Position(0, 8);

        const items = await provideCompletionItems(doc, position);

        expect(items.some((i: any) => i.label === 'random.guid()')).toBe(true);
    });

    test('returns builtin functions when listVariables throws', async () => {
        (cliService.listVariables as jest.Mock).mockRejectedValue(new Error('CLI error'));

        const doc = makeDocument(['let a = ']);
        const position = new vscode.Position(0, 8);

        const items = await provideCompletionItems(doc, position);

        expect(items.some((i: any) => i.label === 'random.guid()')).toBe(true);
    });

    test('triggers for hyphenated variable names', async () => {
        (cliService.listVariables as jest.Mock).mockResolvedValue([
            { name: 'base_url', value: 'http://localhost', file: '/workspace/shared.rq', line: 0, character: 0, source: 'let' }
        ]);

        const doc = makeDocument(['let my-var = ']);
        const position = new vscode.Position(0, 13);

        const items = await provideCompletionItems(doc, position);

        expect(cliService.listVariables).toHaveBeenCalled();
        expect(items.find((i: any) => i.label === 'base_url')).toBeDefined();
    });

    test('does not trigger when not a let assignment', async () => {
        const doc = makeDocument(['let a']);
        const position = new vscode.Position(0, 5);

        await provideCompletionItems(doc, position);

        expect(cliService.listVariables).not.toHaveBeenCalled();
    });

    test('suggests variables when partial word already typed after =', async () => {
        (cliService.listVariables as jest.Mock).mockResolvedValue([
            { name: 'base_url', value: 'http://localhost', file: '/workspace/shared.rq', line: 0, character: 0, source: 'let' },
            { name: 'token', value: 'abc123', file: '/workspace/shared.rq', line: 1, character: 0, source: 'let' }
        ]);

        const doc = makeDocument(['let my_var = bas']);
        const position = new vscode.Position(0, 16);

        const items = await provideCompletionItems(doc, position);

        expect(items).toBeDefined();
        const target = items.find((i: any) => i.label === 'base_url');
        expect(target).toBeDefined();
        expect(target.range.start.character).toBe(13);
        expect(target.range.end.character).toBe(16);
    });
});

describe('variable interpolation completion', () => {
    test('suggests variables with closing braces when typing "{{"', async () => {
        (cliService.listVariables as jest.Mock).mockResolvedValue([
            { name: 'base_url', value: 'http://localhost', file: '/workspace/shared.rq', line: 0, character: 0, source: 'let' },
            { name: 'token', value: 'abc123', file: '/workspace/shared.rq', line: 1, character: 0, source: 'let' }
        ]);

        const doc = makeDocument(['let b = "{{']);
        const position = new vscode.Position(0, 11);

        const items = await provideCompletionItems(doc, position);

        expect(cliService.listVariables).toHaveBeenCalledWith('/workspace/current.rq', undefined);
        expect(items).toHaveLength(2);
        expect(items[0].label).toBe('base_url');
        expect(items[0].insertText).toBe('base_url');
        expect(items[0].detail).toBe('= http://localhost');
    });

    test('returns undefined when no variables exist anywhere', async () => {
        (cliService.listVariables as jest.Mock).mockResolvedValue([]);

        const doc = makeDocument(['rq req("{{']);
        const position = new vscode.Position(0, 10);

        const items = await provideCompletionItems(doc, position);

        expect(items).toBeUndefined();
    });

    test('returns undefined when listVariables throws and no local variables exist', async () => {
        (cliService.listVariables as jest.Mock).mockRejectedValue(new Error('CLI error'));

        const doc = makeDocument(['rq req("{{']);
        const position = new vscode.Position(0, 10);

        const items = await provideCompletionItems(doc, position);

        expect(items).toBeUndefined();
    });

    test('also triggers inside rq property values', async () => {
        (cliService.listVariables as jest.Mock).mockResolvedValue([
            { name: 'token', value: 'abc123', file: '/workspace/shared.rq', line: 0, character: 0, source: 'let' }
        ]);

        const doc = makeDocument(['  headers: ["Authorization": "Bearer {{']);
        const position = new vscode.Position(0, 39);

        const items = await provideCompletionItems(doc, position);

        expect(items).toHaveLength(1);
        expect(items[0].insertText).toBe('token');
    });
});

describe('env/auth block property value completion', () => {
    const mockVars = [
        { name: 'base_url', value: 'http://localhost', file: '/workspace/shared.rq', line: 0, character: 0, source: 'let' },
        { name: 'token', value: 'abc123', file: '/workspace/shared.rq', line: 1, character: 0, source: 'env' }
    ];

    test('suggests variables in env block after prop: "', async () => {
        (cliService.listVariables as jest.Mock).mockResolvedValue(mockVars);

        const lines = ['env dev {', '    api_url: "'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        expect(items.find((i: any) => i.label === 'base_url')?.insertText).toBe('base_url');
        expect(items.find((i: any) => i.label === 'token')?.insertText).toBe('token');
    });

    test('suggests variables in env block after prop: (space, no quote)', async () => {
        (cliService.listVariables as jest.Mock).mockResolvedValue(mockVars);

        const lines = ['env staging {', '    api_url: '];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        expect(items.find((i: any) => i.label === 'base_url')?.insertText).toBe('base_url');
    });

    test('suggests variables in auth block after prop: "', async () => {
        (cliService.listVariables as jest.Mock).mockResolvedValue(mockVars);

        const lines = ['auth bearer_auth(auth_type.bearer) {', '    token: "'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        expect(items.find((i: any) => i.label === 'base_url')?.insertText).toBe('base_url');
    });

    test('uses detail from value field', async () => {
        (cliService.listVariables as jest.Mock).mockResolvedValue(mockVars);

        const lines = ['env dev {', '    api_url: "'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        expect(items.find((i: any) => i.label === 'base_url')?.detail).toBe('= http://localhost');
        expect(items.find((i: any) => i.label === 'token')?.detail).toBe('= abc123');
    });

    test('does not trigger when value contains non-word chars like /', async () => {
        (cliService.listVariables as jest.Mock).mockResolvedValue(mockVars);

        const lines = ['env dev {', '    api_url: "http://'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        expect(items).toBeUndefined();
    });

    test('suggests variables when partial word already typed in env block prop value', async () => {
        (cliService.listVariables as jest.Mock).mockResolvedValue(mockVars);

        const lines = ['env dev {', '    api_url: "bas'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        expect(items).toBeDefined();
        const target = items.find((i: any) => i.label === 'base_url');
        expect(target).toBeDefined();
        expect(target.range.start.character).toBe(14);
        expect(target.range.end.character).toBe(17);
    });

    test('does not trigger outside env/auth blocks', async () => {
        (cliService.listVariables as jest.Mock).mockResolvedValue(mockVars);

        const lines = ['let x = "val"', '    some_prop: "'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        expect(items).toBeUndefined();
    });

    test('triggers for second property when first contains {{value}}', async () => {
        (cliService.listVariables as jest.Mock).mockResolvedValue(mockVars);

        const lines = ['env my_env {', '    var1: "{{base_url}}",', '    var2: "'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(2, lines[2].length);

        const items = await provideCompletionItems(doc, position);

        expect(items.find((i: any) => i.label === 'base_url')?.insertText).toBe('base_url');
    });

    test('suggests variables inside array literal key-value', async () => {
        (cliService.listVariables as jest.Mock).mockResolvedValue(mockVars);

        const lines = ['let my_header = $[', '    "key": "'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        expect(items.find((i: any) => i.label === 'base_url')?.insertText).toBe('base_url');
    });
});

describe('namespace function completion', () => {
    test('suggests io.read_file when typing "io."', async () => {
        const doc = makeDocument(['let body = io.']);
        const position = new vscode.Position(0, 14);

        const items = await provideCompletionItems(doc, position);

        expect(items?.find((i: any) => i.label === 'read_file')).toBeDefined();
    });

    test('suggests read_file when partial already typed after io.', async () => {
        const doc = makeDocument(['let body = io.rea']);
        const position = new vscode.Position(0, 17);

        const items = await provideCompletionItems(doc, position);

        expect(items).toBeDefined();
        const target = items?.find((i: any) => i.label === 'read_file');
        expect(target).toBeDefined();
        expect(target.range.start.character).toBe(14);
        expect(target.range.end.character).toBe(17);
    });

    test('suggests guid when partial already typed after random.', async () => {
        const doc = makeDocument(['let id = random.gu']);
        const position = new vscode.Position(0, 18);

        const items = await provideCompletionItems(doc, position);

        expect(items).toBeDefined();
        const target = items?.find((i: any) => i.label === 'guid');
        expect(target).toBeDefined();
        expect(target.range.start.character).toBe(16);
        expect(target.range.end.character).toBe(18);
    });

    test('suggests now when partial already typed after datetime.', async () => {
        const doc = makeDocument(['let ts = datetime.no']);
        const position = new vscode.Position(0, 20);

        const items = await provideCompletionItems(doc, position);

        expect(items).toBeDefined();
        const target = items?.find((i: any) => i.label === 'now');
        expect(target).toBeDefined();
        expect(target.range.start.character).toBe(18);
        expect(target.range.end.character).toBe(20);
    });
});
