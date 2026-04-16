jest.mock('../../src/rqClient');
jest.mock('../../src/utils', () => ({
    ...jest.requireActual('../../src/utils'),
    mirrorToTemp: jest.fn().mockReturnValue('/tmp/rq-check-mock'),
}));

import * as vscode from 'vscode';
import * as cliService from '../../src/rqClient';
import * as utils from '../../src/utils';
import '../../src/language/completionProvider';

function makeDocument(lines: string[], uri: any = { fsPath: '/workspace/current.rq' }) {
    const fullText = lines.join('\n');
    const lineOffsets: number[] = [];
    let off = 0;
    for (const line of lines) {
        lineOffsets.push(off);
        off += line.length + 1;
    }
    const getText = (range?: any): string => {
        if (!range) { return fullText; }
        const startLine = range.start?.line ?? 0;
        const startChar = range.start?.character ?? 0;
        const endLine = range.end?.line ?? lines.length - 1;
        const endChar = range.end?.character ?? (lines[lines.length - 1]?.length ?? 0);
        const startOffset = (lineOffsets[startLine] ?? 0) + startChar;
        const endOffset = (lineOffsets[endLine] ?? 0) + endChar;
        return fullText.slice(startOffset, endOffset);
    };
    return {
        uri,
        lineCount: lines.length,
        lineAt: (i: number | vscode.Position) => {
            const idx = typeof i === 'number' ? i : (i as vscode.Position).line;
            return { text: lines[idx] };
        },
        getText: jest.fn().mockImplementation(getText),
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

        expect(items.find((i: any) => i.label === 'base_url')?.insertText).toBe('base_url');
    });

    test('suggests variables inside array literal key-value', async () => {
        (cliService.listVariables as jest.Mock).mockResolvedValue(mockVars);

        const lines = ['let my_header = [', '    "key": "'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        expect(items.find((i: any) => i.label === 'base_url')?.insertText).toBe('base_url');
    });
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

        expect(items.find((i: any) => i.label === 'base_url')?.insertText).toBe('base_url');
    });
});

describe('auth keyword snippets', () => {
    test('does not auto-suggest snippets on a blank line when triggered by Enter key', async () => {
        const doc = makeDocument(['']);
        const position = new vscode.Position(0, 0);
        const context = { triggerKind: vscode.CompletionTriggerKind.TriggerCharacter };

        const items = await provideCompletionItems(doc, position, undefined, context);

        expect(items).toBeUndefined();
    });

    test('suggests all top-level snippets on blank line with manual invoke (Ctrl+Space)', async () => {
        const doc = makeDocument(['']);
        const position = new vscode.Position(0, 0);
        const context = { triggerKind: vscode.CompletionTriggerKind.Invoke };

        const items = await provideCompletionItems(doc, position, undefined, context);

        expect(items?.find((i: any) => i.label === 'rq')).toBeDefined();
        expect(items?.find((i: any) => i.label === 'rq …')).toBeDefined();
        expect(items?.find((i: any) => i.label === 'ep')).toBeDefined();
        expect(items?.find((i: any) => i.label === 'ep …')).toBeDefined();
        expect(items?.find((i: any) => i.label === 'env')).toBeDefined();
        expect(items?.find((i: any) => i.label === 'env …')).toBeDefined();
        expect(items?.find((i: any) => i.label === 'auth bearer')).toBeDefined();
        expect(items?.find((i: any) => i.label === 'auth …')).toBeUndefined();
    });

    test('suggests top-level snippets when typing a partial keyword', async () => {
        const doc = makeDocument(['r']);
        const position = new vscode.Position(0, 1);

        const items = await provideCompletionItems(doc, position);

        expect(items?.find((i: any) => i.label === 'rq')).toBeDefined();
        expect(items?.find((i: any) => i.label === 'rq …')).toBeDefined();
        expect(items?.find((i: any) => i.label === 'ep')).toBeDefined();
        expect(items?.find((i: any) => i.label === 'auth bearer')).toBeDefined();
    });

    test('suggests bearer auth snippet when typing auth keyword', async () => {
        const doc = makeDocument(['auth']);
        const position = new vscode.Position(0, 4);

        const items = await provideCompletionItems(doc, position);

        const bearer = items.find((i: any) => i.label === 'auth bearer');
        expect(bearer).toBeDefined();
        expect(bearer.detail).toBe('Auth block — Bearer Token');
        expect(bearer.insertText.value).toContain('auth_type.bearer');
        expect(bearer.insertText.value).toContain('token:');
    });

    test('suggests oauth2_client_credentials client_secret snippet when typing auth keyword', async () => {
        const doc = makeDocument(['auth']);
        const position = new vscode.Position(0, 4);

        const items = await provideCompletionItems(doc, position);

        const target = items.find((i: any) => i.label === 'auth oauth2_client_credentials (client_secret)');
        expect(target).toBeDefined();
        expect(target.detail).toBe('Auth block — OAuth2 Client Credentials (client secret)');
        expect(target.insertText.value).toContain('auth_type.oauth2_client_credentials');
        expect(target.insertText.value).toContain('client_secret:');
        expect(target.insertText.value).toContain('token_url:');
    });

    test('suggests oauth2_client_credentials cert_file snippet when typing auth keyword', async () => {
        const doc = makeDocument(['auth']);
        const position = new vscode.Position(0, 4);

        const items = await provideCompletionItems(doc, position);

        const target = items.find((i: any) => i.label === 'auth oauth2_client_credentials (cert_file)');
        expect(target).toBeDefined();
        expect(target.detail).toBe('Auth block — OAuth2 Client Credentials (certificate)');
        expect(target.insertText.value).toContain('auth_type.oauth2_client_credentials');
        expect(target.insertText.value).toContain('cert_file:');
        expect(target.insertText.value).toContain('token_url:');
    });

    test('suggests oauth2_authorization_code auth snippet when typing auth keyword', async () => {
        const doc = makeDocument(['auth']);
        const position = new vscode.Position(0, 4);

        const items = await provideCompletionItems(doc, position);

        const target = items.find((i: any) => i.label === 'auth oauth2_authorization_code');
        expect(target).toBeDefined();
        expect(target.detail).toBe('Auth block — OAuth2 Authorization Code with PKCE');
        expect(target.insertText.value).toContain('auth_type.oauth2_authorization_code');
        expect(target.insertText.value).toContain('client_id:');
        expect(target.insertText.value).toContain('authorization_url:');
        expect(target.insertText.value).toContain('token_url:');
        expect(target.insertText.value).toContain('redirect_uri:');
        expect(target.insertText.value).not.toContain('code_challenge_method:');
    });

    test('suggests oauth2_implicit auth snippet when typing auth keyword', async () => {
        const doc = makeDocument(['auth']);
        const position = new vscode.Position(0, 4);

        const items = await provideCompletionItems(doc, position);

        const target = items.find((i: any) => i.label === 'auth oauth2_implicit');
        expect(target).toBeDefined();
        expect(target.detail).toBe('Auth block — OAuth2 Implicit Flow');
        expect(target.insertText.value).toContain('auth_type.oauth2_implicit');
        expect(target.insertText.value).toContain('client_id:');
        expect(target.insertText.value).toContain('authorization_url:');
        expect(target.insertText.value).toContain('scope:');
    });

    test('suggests rq keyword item with Keyword kind when typing rq', async () => {
        const doc = makeDocument(['rq']);
        const position = new vscode.Position(0, 2);

        const items = await provideCompletionItems(doc, position);

        const kwItems = items.filter((i: any) => i.label === 'rq');
        const kw = kwItems.find((i: any) => i.kind === vscode.CompletionItemKind.Keyword);
        expect(kw).toBeDefined();
        expect(kw.insertText).toBe('rq');
    });

    test('suggests rq snippet item with Module kind when typing rq', async () => {
        const doc = makeDocument(['rq']);
        const position = new vscode.Position(0, 2);

        const items = await provideCompletionItems(doc, position);

        const snip = items.find((i: any) => i.label === 'rq …');
        expect(snip).toBeDefined();
        expect(snip.kind).toBe(vscode.CompletionItemKind.Module);
        expect(snip.insertText.value).toContain('rq_name');
    });

    test('suggests ep crud snippet with linked widget placeholder', async () => {
        const doc = makeDocument(['ep']);
        const position = new vscode.Position(0, 2);

        const items = await provideCompletionItems(doc, position);

        const snip = items.find((i: any) => i.label === 'ep crud');
        expect(snip).toBeDefined();
        expect(snip.kind).toBe(vscode.CompletionItemKind.Module);
        const val = snip.insertText.value;
        expect(val).toContain('${1:endpoint}_id');
        expect(val).toContain('ep ${1:endpoint}s(');
        expect(val).toContain('${1:endpoint}-post.json');
        expect(val).toContain('${1:endpoint}-patch.json');
        expect(val).toContain('${1:endpoint}_id');
        expect(val).toContain('rq list()');
        expect(val).toContain('rq get()');
        expect(val).toContain('rq post(');
        expect(val).toContain('rq patch(');
        expect(val).toContain('rq delete()');
    });

    test('suggests ep keyword item with Keyword kind when typing ep', async () => {
        const doc = makeDocument(['ep']);
        const position = new vscode.Position(0, 2);

        const items = await provideCompletionItems(doc, position);

        const kw = items.filter((i: any) => i.label === 'ep').find((i: any) => i.kind === vscode.CompletionItemKind.Keyword);
        expect(kw).toBeDefined();
    });

    test('suggests env keyword item with Keyword kind when typing env', async () => {
        const doc = makeDocument(['env']);
        const position = new vscode.Position(0, 3);

        const items = await provideCompletionItems(doc, position);

        const kw = items.filter((i: any) => i.label === 'env').find((i: any) => i.kind === vscode.CompletionItemKind.Keyword);
        expect(kw).toBeDefined();
    });

    test('suggests auth keyword item with Keyword kind when typing auth', async () => {
        const doc = makeDocument(['auth']);
        const position = new vscode.Position(0, 4);

        const items = await provideCompletionItems(doc, position);

        const kw = items.filter((i: any) => i.label === 'auth').find((i: any) => i.kind === vscode.CompletionItemKind.Keyword);
        expect(kw).toBeDefined();
        expect(kw.insertText).toBe('auth');
    });

    test('suggests let keyword item with Keyword kind in top-level section', async () => {
        const doc = makeDocument(['l']);
        const position = new vscode.Position(0, 1);

        const items = await provideCompletionItems(doc, position);

        const kw = items.find((i: any) => i.label === 'let' && i.kind === vscode.CompletionItemKind.Keyword);
        expect(kw).toBeDefined();
    });

    test('suggests import keyword item with Keyword kind in top-level section', async () => {
        const doc = makeDocument(['im']);
        const position = new vscode.Position(0, 2);

        const items = await provideCompletionItems(doc, position);

        const kw = items.find((i: any) => i.label === 'import' && i.kind === vscode.CompletionItemKind.Keyword);
        expect(kw).toBeDefined();
    });

    test('all auth snippet variants have Module kind', async () => {
        const doc = makeDocument(['auth']);
        const position = new vscode.Position(0, 4);

        const items = await provideCompletionItems(doc, position);

        const snippetLabels = ['auth bearer', 'auth oauth2_client_credentials (client_secret)', 'auth oauth2_client_credentials (cert_file)', 'auth oauth2_authorization_code', 'auth oauth2_implicit'];
        snippetLabels.forEach(label => {
            const item = items.find((i: any) => i.label === label);
            expect(item).toBeDefined();
            expect(item.kind).toBe(vscode.CompletionItemKind.Module);
        });
    });

    test('suggests import keyword at start of file', async () => {
        const doc = makeDocument(['']);
        const position = new vscode.Position(0, 0);

        const items = await provideCompletionItems(doc, position);

        expect(items?.find((i: any) => i.label === 'import')).toBeDefined();
    });

    test('suggests import keyword after another import', async () => {
        const lines = ['import "shared";', ''];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, 0);

        const items = await provideCompletionItems(doc, position);

        expect(items?.find((i: any) => i.label === 'import')).toBeDefined();
    });

    test('hides import keyword after let declaration', async () => {
        const lines = ['let base = "http://localhost";', ''];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, 0);

        const items = await provideCompletionItems(doc, position);

        expect(items?.find((i: any) => i.label === 'import')).toBeUndefined();
    });

    test('hides import keyword after rq statement', async () => {
        const lines = ['rq get("http://localhost");', ''];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, 0);

        const items = await provideCompletionItems(doc, position);

        expect(items?.find((i: any) => i.label === 'import')).toBeUndefined();
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

    test('inside ep body suggests only rq keyword and snippet', async () => {
        const lines = ['ep api("http://localhost") {', '    '];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        expect(items?.find((i: any) => i.label === 'rq')).toBeDefined();
        expect(items?.find((i: any) => i.label === 'rq …')).toBeDefined();
        expect(items?.find((i: any) => i.label === 'env')).toBeUndefined();
        expect(items?.find((i: any) => i.label === 'auth')).toBeUndefined();
        expect(items?.find((i: any) => i.label === 'ep')).toBeUndefined();
        expect(items?.find((i: any) => i.label === 'let')).toBeUndefined();
        expect(items?.find((i: any) => i.label === 'import')).toBeUndefined();
    });

    test('after ep body closes suggests all root-level keywords again', async () => {
        const lines = ['ep api("http://localhost") {', '  rq list();', '}', ''];
        const doc = makeDocument(lines);
        const position = new vscode.Position(3, 0);

        const items = await provideCompletionItems(doc, position);

        expect(items?.find((i: any) => i.label === 'env')).toBeDefined();
        expect(items?.find((i: any) => i.label === 'auth')).toBeDefined();
        expect(items?.find((i: any) => i.label === 'ep')).toBeDefined();
    });

});

describe('auth declaration parameter completion', () => {
    test('suggests auth_type when opening paren is typed', async () => {
        const lines = ['auth my_auth('];
        const doc = makeDocument(lines);
        const position = new vscode.Position(0, lines[0].length);

        const items = await provideCompletionItems(doc, position);

        expect(items).toHaveLength(1);
        expect(items[0].label).toBe('auth_type');
        expect(items[0].kind).toBe(vscode.CompletionItemKind.EnumMember);
        expect(items[0].insertText.value).toBe('auth_type.');
        expect(items[0].command?.command).toBe('editor.action.triggerSuggest');
    });

    test('suggests auth_type on blank inner text', async () => {
        const lines = ['auth my_auth( '];
        const doc = makeDocument(lines);
        const position = new vscode.Position(0, lines[0].length);

        const items = await provideCompletionItems(doc, position);

        expect(items).toHaveLength(1);
        expect(items[0].label).toBe('auth_type');
    });

    test('does not suggest auth_type when auth_type. already typed', async () => {
        const lines = ['auth my_auth(auth_type.'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(0, lines[0].length);

        const items = await provideCompletionItems(doc, position);

        expect(items?.find((i: any) => i.label === 'auth_type')).toBeUndefined();
        expect(items?.some((i: any) => ['bearer', 'oauth2_client_credentials', 'oauth2_authorization_code', 'oauth2_implicit'].includes(i.label))).toBe(true);
    });

    test('does not suggest auth_type inside auth body block', async () => {
        const lines = ['auth my_auth(auth_type.bearer) {', '    '];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        expect(items?.find((i: any) => i.label === 'auth_type')).toBeUndefined();
    });
});

describe('auth block property name completion', () => {
    test('suggests all bearer properties on empty line inside bearer block', async () => {
        const lines = ['auth my_auth(auth_type.bearer) {', '    '];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        expect(items.find((i: any) => i.label === 'token')).toBeDefined();
    });

    test('suggests required oauth2_client_credentials properties', async () => {
        const lines = ['auth my_auth(auth_type.oauth2_client_credentials) {', '    '];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        expect(items.find((i: any) => i.label === 'client_id')?.detail).toBe('required');
        expect(items.find((i: any) => i.label === 'token_url')?.detail).toBe('required');
        expect(items.find((i: any) => i.label === 'client_secret')?.detail).toBe('optional');
        expect(items.find((i: any) => i.label === 'scope')?.detail).toBe('optional');
    });

    test('excludes already defined properties', async () => {
        const lines = [
            'auth my_auth(auth_type.oauth2_client_credentials) {',
            '    client_id: "my-id",',
            '    '
        ];
        const doc = makeDocument(lines);
        const position = new vscode.Position(2, lines[2].length);

        const items = await provideCompletionItems(doc, position);

        expect(items.find((i: any) => i.label === 'client_id')).toBeUndefined();
        expect(items.find((i: any) => i.label === 'token_url')).toBeDefined();
    });

    test('suggests oauth2_authorization_code properties', async () => {
        const lines = ['auth my_auth(auth_type.oauth2_authorization_code) {', '    '];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        expect(items.find((i: any) => i.label === 'client_id')).toBeDefined();
        expect(items.find((i: any) => i.label === 'authorization_url')).toBeDefined();
        expect(items.find((i: any) => i.label === 'token_url')).toBeDefined();
        expect(items.find((i: any) => i.label === 'redirect_uri')).toBeDefined();
        expect(items.find((i: any) => i.label === 'scope')).toBeDefined();
        expect(items.find((i: any) => i.label === 'code_challenge_method')).toBeDefined();
    });

    test('suggests oauth2_implicit properties', async () => {
        const lines = ['auth my_auth(auth_type.oauth2_implicit) {', '    '];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        expect(items.find((i: any) => i.label === 'client_id')).toBeDefined();
        expect(items.find((i: any) => i.label === 'authorization_url')).toBeDefined();
        expect(items.find((i: any) => i.label === 'redirect_uri')).toBeDefined();
        expect(items.find((i: any) => i.label === 'scope')).toBeDefined();
    });

    test('does not trigger outside an auth block', async () => {
        const lines = ['env dev {', '    '];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        expect(items?.find((i: any) => i.label === 'token')).toBeUndefined();
    });

    test('suggests S256 and plain for code_challenge_method value with opening quote, closes the string', async () => {
        const lines = ['auth my_auth(auth_type.oauth2_authorization_code) {', '    code_challenge_method: "'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        const s256 = items.find((i: any) => i.label === 'S256');
        const plain = items.find((i: any) => i.label === 'plain');
        expect(s256).toBeDefined();
        expect(s256.insertText).toBe('S256"');
        expect(plain).toBeDefined();
        expect(plain.insertText).toBe('plain"');
    });

    test('suggests S256 and plain for code_challenge_method value without opening quote, wraps in quotes', async () => {
        const lines = ['auth my_auth(auth_type.oauth2_authorization_code) {', '    code_challenge_method: '];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        const s256 = items.find((i: any) => i.label === 'S256');
        const plain = items.find((i: any) => i.label === 'plain');
        expect(s256).toBeDefined();
        expect(s256.insertText).toBe('"S256"');
        expect(plain).toBeDefined();
        expect(plain.insertText).toBe('"plain"');
    });

    test('code_challenge_method property completion uses choice snippet', async () => {
        const lines = ['auth my_auth(auth_type.oauth2_authorization_code) {', '    '];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, lines[1].length);

        const items = await provideCompletionItems(doc, position);

        const target = items.find((i: any) => i.label === 'code_challenge_method');
        expect(target).toBeDefined();
        expect(target.insertText.value).toContain('${1|S256,plain|}');
    });

    test('triggers on blank new line after enter (newline trigger)', async () => {
        const lines = ['auth my_auth(auth_type.oauth2_client_credentials) {', '    client_id: "my-id",', '    '];
        const doc = makeDocument(lines);
        const position = new vscode.Position(2, lines[2].length);

        const items = await provideCompletionItems(doc, position);

        expect(items.find((i: any) => i.label === 'token_url')).toBeDefined();
        expect(items.find((i: any) => i.label === 'client_id')).toBeUndefined();
    });
});

describe('resolveCliPath — temp mirroring for unsaved files', () => {
    test('uses original file path when no dirty rq docs exist', async () => {
        (vscode.workspace.textDocuments as any) = [];
        (cliService.listVariables as jest.Mock).mockResolvedValue([
            { name: 'my_var', value: 'x', source: 'let' }
        ]);

        const doc = makeDocument(['let a = ']);
        const position = new vscode.Position(0, 8);

        await provideCompletionItems(doc, position);

        expect(utils.mirrorToTemp).not.toHaveBeenCalled();
        expect(cliService.listVariables).toHaveBeenCalledWith('/workspace/current.rq', undefined);
    });

    test('uses temp path when current document is dirty', async () => {
        const dirtyDoc = {
            languageId: 'rq',
            isDirty: true,
            uri: { fsPath: '/workspace/current.rq' },
            getText: () => 'let a = "dirty"'
        };
        (vscode.workspace.textDocuments as any) = [dirtyDoc];
        (vscode.workspace.getWorkspaceFolder as jest.Mock).mockReturnValue({
            uri: { fsPath: '/workspace' }
        });
        (cliService.listVariables as jest.Mock).mockResolvedValue([]);

        const doc = makeDocument(['let a = ']);
        const position = new vscode.Position(0, 8);

        await provideCompletionItems(doc, position);

        expect(utils.mirrorToTemp).toHaveBeenCalled();
        expect(cliService.listVariables).toHaveBeenCalledWith('/tmp/rq-check-mock/current.rq', undefined);
    });

    test('uses temp path when a different rq file is dirty', async () => {
        const dirtyDoc = {
            languageId: 'rq',
            isDirty: true,
            uri: { fsPath: '/workspace/shared.rq' },
            getText: () => 'let token = "secret"'
        };
        (vscode.workspace.textDocuments as any) = [dirtyDoc];
        (vscode.workspace.getWorkspaceFolder as jest.Mock).mockReturnValue({
            uri: { fsPath: '/workspace' }
        });
        (cliService.listVariables as jest.Mock).mockResolvedValue([]);

        const doc = makeDocument(['let a = ']);
        const position = new vscode.Position(0, 8);

        await provideCompletionItems(doc, position);

        expect(utils.mirrorToTemp).toHaveBeenCalled();
        expect(cliService.listVariables).toHaveBeenCalledWith('/tmp/rq-check-mock/current.rq', undefined);
    });

    test('passes dirty doc content as overrides to mirrorToTemp', async () => {
        const dirtyDoc = {
            languageId: 'rq',
            isDirty: true,
            uri: { fsPath: '/workspace/shared.rq' },
            getText: () => 'let token = "secret"'
        };
        (vscode.workspace.textDocuments as any) = [dirtyDoc];
        (vscode.workspace.getWorkspaceFolder as jest.Mock).mockReturnValue({
            uri: { fsPath: '/workspace' }
        });

        const doc = makeDocument(['let a = ']);
        const position = new vscode.Position(0, 8);

        await provideCompletionItems(doc, position);

        const overrides: Map<string, string> = (utils.mirrorToTemp as jest.Mock).mock.calls[0][1];
        expect(overrides.get('/workspace/shared.rq')).toBe('let token = "secret"');
    });

    test('ignores non-rq dirty documents', async () => {
        const dirtyDoc = {
            languageId: 'typescript',
            isDirty: true,
            uri: { fsPath: '/workspace/extension.ts' },
            getText: () => 'export {}'
        };
        (vscode.workspace.textDocuments as any) = [dirtyDoc];

        const doc = makeDocument(['let a = ']);
        const position = new vscode.Position(0, 8);

        await provideCompletionItems(doc, position);

        expect(utils.mirrorToTemp).not.toHaveBeenCalled();
    });
});

describe('auth attribute value completion', () => {
    test('suggests auth configs when typing [auth("', async () => {
        (cliService.listAuthConfigs as jest.Mock).mockResolvedValue([
            { name: 'my_bearer', auth_type: 'bearer' },
            { name: 'my_oauth', auth_type: 'oauth2_client_credentials' }
        ]);

        const doc = makeDocument(['[auth("']);
        const position = new vscode.Position(0, 7);

        const items = await provideCompletionItems(doc, position);

        expect(cliService.listAuthConfigs).toHaveBeenCalledWith('/workspace/current.rq');
        expect(items).toHaveLength(2);
        expect(items[0].label).toBe('my_bearer');
        expect(items[0].detail).toBe('bearer');
        expect(items[1].label).toBe('my_oauth');
    });

    test('uses temp path for auth configs when docs are dirty', async () => {
        const dirtyDoc = {
            languageId: 'rq',
            isDirty: true,
            uri: { fsPath: '/workspace/auth.rq' },
            getText: () => 'auth my_bearer(auth_type.bearer) { token: "t" }'
        };
        (vscode.workspace.textDocuments as any) = [dirtyDoc];
        (vscode.workspace.getWorkspaceFolder as jest.Mock).mockReturnValue({
            uri: { fsPath: '/workspace' }
        });
        (cliService.listAuthConfigs as jest.Mock).mockResolvedValue([
            { name: 'my_bearer', auth_type: 'bearer' }
        ]);

        const doc = makeDocument(['[auth("']);
        const position = new vscode.Position(0, 7);

        await provideCompletionItems(doc, position);

        expect(utils.mirrorToTemp).toHaveBeenCalled();
        expect(cliService.listAuthConfigs).toHaveBeenCalledWith('/tmp/rq-check-mock/current.rq');
    });

    test('returns undefined when listAuthConfigs throws', async () => {
        (cliService.listAuthConfigs as jest.Mock).mockRejectedValue(new Error('CLI error'));

        const doc = makeDocument(['[auth("']);
        const position = new vscode.Position(0, 7);

        const items = await provideCompletionItems(doc, position);

        expect(items).toBeUndefined();
    });

    test('does not trigger outside auth attribute context', async () => {
        const doc = makeDocument(['let x = "']);
        const position = new vscode.Position(0, 9);

        await provideCompletionItems(doc, position);

        expect(cliService.listAuthConfigs).not.toHaveBeenCalled();
    });
});

describe('attribute completion', () => {
    test('suggests method, timeout, auth when [ typed at start of line', async () => {
        const doc = makeDocument(['[']);
        const position = new vscode.Position(0, 1);

        const items = await provideCompletionItems(doc, position);

        expect(items.find((i: any) => i.label === 'method')).toBeDefined();
        expect(items.find((i: any) => i.label === 'timeout')).toBeDefined();
        expect(items.find((i: any) => i.label === 'auth')).toBeDefined();
    });

    test('does not suggest attributes inside a header dict', async () => {
        const lines = ['let h = [', '    "'];
        const doc = makeDocument(lines);
        const position = new vscode.Position(1, 5);

        const items = await provideCompletionItems(doc, position);

        expect(items?.find((i: any) => i.label === 'method')).toBeUndefined();
    });

    test('method item uses snippet with HTTP verb choices', async () => {
        const doc = makeDocument(['[']);
        const position = new vscode.Position(0, 1);

        const items = await provideCompletionItems(doc, position);

        const method = items.find((i: any) => i.label === 'method');
        expect((method.insertText as any).value).toContain('GET,POST,PUT,DELETE');
    });

    test('auth item triggers suggest command after insertion', async () => {
        const doc = makeDocument(['[']);
        const position = new vscode.Position(0, 1);

        const items = await provideCompletionItems(doc, position);

        const auth = items.find((i: any) => i.label === 'auth');
        expect(auth.command?.command).toBe('editor.action.triggerSuggest');
    });
});
