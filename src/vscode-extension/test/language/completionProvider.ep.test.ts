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

        await provideCompletionItems(doc, position);

        expect(cliService.listEndpoints).not.toHaveBeenCalled();
    });
});
