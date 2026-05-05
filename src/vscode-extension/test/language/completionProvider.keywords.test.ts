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

describe('top-level keyword completion', () => {
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
        expect(kw.insertText).toBe('rq ');
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
        expect(kw.insertText).toBe('auth ');
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

        const snippetLabels = [
            'auth bearer',
            'auth oauth2_client_credentials (client_secret)',
            'auth oauth2_client_credentials (cert_file)',
            'auth oauth2_authorization_code',
            'auth oauth2_implicit'
        ];
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
