jest.mock('../../src/cliService');

import * as vscode from 'vscode';
import * as cliService from '../../src/cliService';
import '../../src/language/completionProvider';

function makeDocument(lines: string[], uri: any = { fsPath: '/workspace/current.rq' }) {
    return {
        uri,
        lineCount: lines.length,
        lineAt: (i: number | vscode.Position) => {
            const idx = typeof i === 'number' ? i : (i as vscode.Position).line;
            return { text: lines[idx] };
        },
        getText: jest.fn().mockReturnValue(lines.join('\n')),
        getWordRangeAtPosition: jest.fn()
    };
}

let provideCompletionItems: Function;

beforeAll(() => {
    const calls = (vscode.languages.registerCompletionItemProvider as jest.Mock).mock.calls;
    provideCompletionItems = calls[calls.length - 1][1].provideCompletionItems;
});

beforeEach(() => {
    jest.clearAllMocks();
    (vscode.workspace.findFiles as jest.Mock).mockResolvedValue([]);
    (cliService.listEndpoints as jest.Mock).mockResolvedValue([]);
    (cliService.listVariables as jest.Mock).mockResolvedValue([]);
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
});

describe('ep template completion', () => {
    test('suggests template endpoints when typing "ep name<"', async () => {
        (cliService.listEndpoints as jest.Mock).mockResolvedValue([
            { name: 'api', file: '/workspace/api.rq', line: 0, character: 0, is_template: true },
            { name: 'base', file: '/workspace/base.rq', line: 0, character: 0, is_template: true }
        ]);

        const doc = makeDocument(['ep my_ep<']);
        const position = new vscode.Position(0, 9);

        const items = await provideCompletionItems(doc, position);

        expect(cliService.listEndpoints).toHaveBeenCalledWith('/workspace/current.rq');
        expect(items).toHaveLength(2);
        expect(items[0].label).toBe('api');
        expect(items[0].insertText).toBe('api');
        expect(items[1].label).toBe('base');
    });

    test('excludes non-template endpoints', async () => {
        (cliService.listEndpoints as jest.Mock).mockResolvedValue([
            { name: 'api', file: '/workspace/api.rq', line: 0, character: 0, is_template: true },
            { name: 'full', file: '/workspace/full.rq', line: 0, character: 0, is_template: false }
        ]);

        const doc = makeDocument(['ep my_ep<']);
        const position = new vscode.Position(0, 9);

        const items = await provideCompletionItems(doc, position);

        expect(items).toHaveLength(1);
        expect(items[0].label).toBe('api');
    });

    test('returns empty list when no template endpoints exist', async () => {
        (cliService.listEndpoints as jest.Mock).mockResolvedValue([
            { name: 'full', file: '/workspace/full.rq', line: 0, character: 0, is_template: false }
        ]);

        const doc = makeDocument(['ep my_ep<']);
        const position = new vscode.Position(0, 9);

        const items = await provideCompletionItems(doc, position);

        expect(items).toHaveLength(0);
    });

    test('returns undefined when listEndpoints throws', async () => {
        (cliService.listEndpoints as jest.Mock).mockRejectedValue(new Error('CLI error'));

        const doc = makeDocument(['ep my_ep<']);
        const position = new vscode.Position(0, 9);

        const items = await provideCompletionItems(doc, position);

        expect(items).toBeUndefined();
    });

    test('does not trigger when < is not after ep name', async () => {
        (cliService.listEndpoints as jest.Mock).mockResolvedValue([
            { name: 'api', file: '/workspace/api.rq', line: 0, character: 0, is_template: true }
        ]);

        const doc = makeDocument(['let x = "<"']);
        const position = new vscode.Position(0, 11);

        const items = await provideCompletionItems(doc, position);

        expect(cliService.listEndpoints).not.toHaveBeenCalled();
    });
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

        expect(cliService.listVariables).toHaveBeenCalledWith('/workspace/current.rq');
        expect(items).toHaveLength(2);
        expect(items[0].label).toBe('base_url');
        expect(items[0].insertText).toBe('base_url;');
        expect(items[0].detail).toBe('= http://localhost');
        expect(items[1].label).toBe('token');
    });

    test('uses source as detail when value is empty', async () => {
        (cliService.listVariables as jest.Mock).mockResolvedValue([
            { name: 'my_var', value: '', file: '/workspace/shared.rq', line: 0, character: 0, source: 'let' }
        ]);

        const doc = makeDocument(['let result = ']);
        const position = new vscode.Position(0, 13);

        const items = await provideCompletionItems(doc, position);

        expect(items).toHaveLength(1);
        expect(items[0].detail).toBe('let');
    });

    test('returns undefined when no variables exist', async () => {
        (cliService.listVariables as jest.Mock).mockResolvedValue([]);

        const doc = makeDocument(['let a = ']);
        const position = new vscode.Position(0, 8);

        const items = await provideCompletionItems(doc, position);

        expect(items).toBeUndefined();
    });

    test('returns undefined when listVariables throws', async () => {
        (cliService.listVariables as jest.Mock).mockRejectedValue(new Error('CLI error'));

        const doc = makeDocument(['let a = ']);
        const position = new vscode.Position(0, 8);

        const items = await provideCompletionItems(doc, position);

        expect(items).toBeUndefined();
    });

    test('triggers for hyphenated variable names', async () => {
        (cliService.listVariables as jest.Mock).mockResolvedValue([
            { name: 'base_url', value: 'http://localhost', file: '/workspace/shared.rq', line: 0, character: 0, source: 'let' }
        ]);

        const doc = makeDocument(['let my-var = ']);
        const position = new vscode.Position(0, 13);

        const items = await provideCompletionItems(doc, position);

        expect(cliService.listVariables).toHaveBeenCalled();
        expect(items).toHaveLength(1);
    });

    test('does not trigger when not a let assignment', async () => {
        const doc = makeDocument(['let a']);
        const position = new vscode.Position(0, 5);

        await provideCompletionItems(doc, position);

        expect(cliService.listVariables).not.toHaveBeenCalled();
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

        expect(cliService.listVariables).toHaveBeenCalledWith('/workspace/current.rq');
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

        expect(items).toHaveLength(2);
        expect(items[0].label).toBe('base_url');
        expect(items[0].insertText).toBe('base_url');
        expect(items[1].insertText).toBe('token');
    });

    test('suggests variables in env block after prop: (space, no quote)', async () => {
        (cliService.listVariables as jest.Mock).mockResolvedValue(mockVars);

        const lines = ['env staging {', '    api_url: '];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        expect(items).toHaveLength(2);
        expect(items[0].insertText).toBe('base_url');
    });

    test('suggests variables in auth block after prop: "', async () => {
        (cliService.listVariables as jest.Mock).mockResolvedValue(mockVars);

        const lines = ['auth bearer_auth(auth_type.bearer) {', '    token: "'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        expect(items).toHaveLength(2);
        expect(items[0].insertText).toBe('base_url');
    });

    test('uses detail from value field', async () => {
        (cliService.listVariables as jest.Mock).mockResolvedValue(mockVars);

        const lines = ['env dev {', '    api_url: "'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        expect(items[0].detail).toBe('= http://localhost');
        expect(items[1].detail).toBe('= abc123');
    });

    test('does not trigger when value already has content', async () => {
        (cliService.listVariables as jest.Mock).mockResolvedValue(mockVars);

        const lines = ['env dev {', '    api_url: "http://'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        expect(items).toBeUndefined();
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

        expect(items).toHaveLength(2);
        expect(items[0].insertText).toBe('base_url');
    });

    test('suggests variables inside array literal key-value', async () => {
        (cliService.listVariables as jest.Mock).mockResolvedValue(mockVars);

        const lines = ['let my_header = [', '    "key": "'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        expect(items).toHaveLength(2);
        expect(items[0].insertText).toBe('base_url');
    });
});

describe('header key completion', () => {
    test('suggests common HTTP headers after [ opening', async () => {
        const lines = ['let my_h = ['];
        const doc = makeDocument(lines);
        const position = new vscode.Position(0, lines[0].length);

        const items = await provideCompletionItems(doc, position);

        expect(items).not.toBeUndefined();
        expect(items.some((i: any) => i.label === 'Content-Type')).toBe(true);
        expect(items.find((i: any) => i.label === 'Content-Type').insertText.value).toBe('"Content-Type": "${1:}"');
    });

    test('suggests headers after opening quote in array', async () => {
        const lines = ['let my_h = ["'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(0, lines[0].length);

        const items = await provideCompletionItems(doc, position);

        expect(items).not.toBeUndefined();
        expect(items.find((i: any) => i.label === 'Authorization').insertText.value).toBe('"Authorization": "${1:}"');
    });

    test('suggests headers on blank new line inside array (after pressing Enter)', async () => {
        const lines = ['    "Content-Type": "application/json",', '    '];
        const doc = makeDocument(lines, { fsPath: '/workspace/current.rq' });
        (doc as any).getText = jest.fn().mockReturnValue('let my_h = [\n' + lines[0] + '\n' + lines[1]);
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

        expect(items).toHaveLength(1);
        expect(items[0].label).toBe('base_url');
        expect(items[0].insertText).toBe('base_url');
    });
});
