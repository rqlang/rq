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
