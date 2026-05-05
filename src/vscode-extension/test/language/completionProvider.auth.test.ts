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
