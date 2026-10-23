jest.mock('../../src/rqClient');

import * as vscode from 'vscode';
import * as cliService from '../../src/rqClient';
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
});

describe('unsaved documents reach rq as drafts', () => {
    test('passes no drafts when no rq doc is dirty', async () => {
        (vscode.workspace.textDocuments as any) = [];

        await provideCompletionItems(makeDocument(['let a = ']), new vscode.Position(0, 8));

        expect(cliService.listVariables).toHaveBeenCalledWith('/workspace/current.rq', undefined, '/workspace', []);
    });

    test('passes the current document as a draft when it is dirty', async () => {
        const dirtyDoc = {
            languageId: 'rq',
            isDirty: true,
            uri: { fsPath: '/workspace/current.rq' },
            getText: () => 'let a = "dirty"'
        };
        (vscode.workspace.textDocuments as any) = [dirtyDoc];
        (vscode.workspace.getWorkspaceFolder as jest.Mock).mockReturnValue({ uri: { fsPath: '/workspace' } });

        await provideCompletionItems(makeDocument(['let a = ']), new vscode.Position(0, 8));

        expect(cliService.listVariables).toHaveBeenCalledWith('/workspace/current.rq', undefined, '/workspace', [
            { path: '/workspace/current.rq', source: 'let a = "dirty"' }
        ]);
    });

    test('passes another dirty rq doc as a draft', async () => {
        const dirtyDoc = {
            languageId: 'rq',
            isDirty: true,
            uri: { fsPath: '/workspace/shared.rq' },
            getText: () => 'let token = "secret"'
        };
        (vscode.workspace.textDocuments as any) = [dirtyDoc];
        (vscode.workspace.getWorkspaceFolder as jest.Mock).mockReturnValue({ uri: { fsPath: '/workspace' } });

        await provideCompletionItems(makeDocument(['let a = ']), new vscode.Position(0, 8));

        expect(cliService.listVariables).toHaveBeenCalledWith('/workspace/current.rq', undefined, '/workspace', [
            { path: '/workspace/shared.rq', source: 'let token = "secret"' }
        ]);
    });

    test('leaves out dirty rq docs from another folder', async () => {
        const dirtyDoc = {
            languageId: 'rq',
            isDirty: true,
            uri: { fsPath: '/elsewhere/shared.rq' },
            getText: () => 'let token = "secret"'
        };
        (vscode.workspace.textDocuments as any) = [dirtyDoc];
        (vscode.workspace.getWorkspaceFolder as jest.Mock).mockReturnValue({ uri: { fsPath: '/workspace' } });

        await provideCompletionItems(makeDocument(['let a = ']), new vscode.Position(0, 8));

        expect(cliService.listVariables).toHaveBeenCalledWith('/workspace/current.rq', undefined, '/workspace', []);
    });

    test('does not read unsaved documents for completions that do not ask rq', async () => {
        const getText = jest.fn(() => 'let token = "secret"');
        (vscode.workspace.textDocuments as any) = [
            { languageId: 'rq', isDirty: true, uri: { fsPath: '/workspace/shared.rq' }, getText }
        ];
        (vscode.workspace.getWorkspaceFolder as jest.Mock).mockReturnValue({ uri: { fsPath: '/workspace' } });

        await provideCompletionItems(makeDocument(['']), new vscode.Position(0, 0));

        expect(cliService.listVariables).not.toHaveBeenCalled();
        expect(getText).not.toHaveBeenCalled();
    });

    test('leaves out dirty rq docs from a sibling folder that shares the prefix', async () => {
        const dirtyDoc = {
            languageId: 'rq',
            isDirty: true,
            uri: { fsPath: '/workspace-other/shared.rq' },
            getText: () => 'let token = "secret"'
        };
        (vscode.workspace.textDocuments as any) = [dirtyDoc];
        (vscode.workspace.getWorkspaceFolder as jest.Mock).mockReturnValue({ uri: { fsPath: '/workspace' } });

        await provideCompletionItems(makeDocument(['let a = ']), new vscode.Position(0, 8));

        expect(cliService.listVariables).toHaveBeenCalledWith('/workspace/current.rq', undefined, '/workspace', []);
    });

    test('ignores non-rq dirty documents', async () => {
        const dirtyDoc = {
            languageId: 'typescript',
            isDirty: true,
            uri: { fsPath: '/workspace/extension.ts' },
            getText: () => 'export {}'
        };
        (vscode.workspace.textDocuments as any) = [dirtyDoc];

        await provideCompletionItems(makeDocument(['let a = ']), new vscode.Position(0, 8));

        expect(cliService.listVariables).toHaveBeenCalledWith('/workspace/current.rq', undefined, '/workspace', []);
    });
});

describe('workspace root seeding for nested files', () => {
    test('seeds variables from the workspace root, not the file directory', async () => {
        (vscode.workspace.getWorkspaceFolder as jest.Mock).mockReturnValue({ uri: { fsPath: '/workspace' } });
        (cliService.listVariables as jest.Mock).mockResolvedValue([
            { name: 'base_url', value: 'https://example.com', source: 'let' }
        ]);

        const doc = makeDocument(['let a = '], { fsPath: '/workspace/inma/collections.rq' });

        await provideCompletionItems(doc, new vscode.Position(0, 8));

        expect(cliService.listVariables).toHaveBeenCalledWith(
            '/workspace/inma/collections.rq',
            undefined,
            '/workspace',
            []
        );
    });

    test('seeds endpoints from the workspace root, not the file directory', async () => {
        (vscode.workspace.getWorkspaceFolder as jest.Mock).mockReturnValue({ uri: { fsPath: '/workspace' } });
        (cliService.listEndpoints as jest.Mock).mockResolvedValue([]);

        const doc = makeDocument(['ep child<'], { fsPath: '/workspace/inma/collections.rq' });

        await provideCompletionItems(doc, new vscode.Position(0, 9));

        expect(cliService.listEndpoints).toHaveBeenCalledWith(
            '/workspace/inma/collections.rq',
            '/workspace',
            []
        );
    });

    test('seeds auth configs from the workspace root, not the file directory', async () => {
        (vscode.workspace.getWorkspaceFolder as jest.Mock).mockReturnValue({ uri: { fsPath: '/workspace' } });
        (cliService.listAuthConfigs as jest.Mock).mockResolvedValue([]);

        const doc = makeDocument(['[auth("'], { fsPath: '/workspace/inma/collections.rq' });

        await provideCompletionItems(doc, new vscode.Position(0, 7));

        expect(cliService.listAuthConfigs).toHaveBeenCalledWith(
            '/workspace/inma/collections.rq',
            '/workspace',
            []
        );
    });
});
