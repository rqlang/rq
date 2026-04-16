jest.mock('../../src/rqClient');

import * as vscode from 'vscode';
import * as cliService from '../../src/rqClient';
import '../../src/language/referenceProvider';

function makeDocument(lines: string[]) {
    return {
        lineAt: (i: number | vscode.Position) => {
            const idx = typeof i === 'number' ? i : (i as vscode.Position).line;
            return { text: lines[idx] };
        },
        getWordRangeAtPosition: jest.fn(),
        getText: jest.fn()
    };
}

let provideReferences: Function;

beforeAll(() => {
    const calls = (vscode.languages.registerReferenceProvider as jest.Mock).mock.calls;
    provideReferences = calls[calls.length - 1][1].provideReferences;
});

beforeEach(() => {
    jest.clearAllMocks();
    (vscode.workspace as any).workspaceFolders = [{ uri: { fsPath: '/workspace' } }];
});

describe('variable references', () => {
    test('calls varRefs with the word at cursor and returns locations', async () => {
        const doc = makeDocument(['rq get("{{base_url}}/v1");']);
        const position = new vscode.Position(0, 10);

        (doc.getWordRangeAtPosition as jest.Mock).mockReturnValue({ start: position, end: position });
        (doc.getText as jest.Mock).mockReturnValue('base_url');
        (cliService.varRefs as jest.Mock).mockResolvedValue([
            { file: '/workspace/a.rq', line: 2, character: 5 },
            { file: '/workspace/b.rq', line: 7, character: 0 }
        ]);

        const result = await provideReferences(doc, position);

        expect(cliService.varRefs).toHaveBeenCalledWith('base_url', '/workspace');
        expect(result).toHaveLength(2);
        expect(result[0]).toBeInstanceOf(vscode.Location);
        expect((result[0] as vscode.Location).range.start).toEqual(new vscode.Position(2, 5));
        expect(result[1]).toBeInstanceOf(vscode.Location);
        expect((result[1] as vscode.Location).range.start).toEqual(new vscode.Position(7, 0));
    });

    test('returns empty array when varRefs throws', async () => {
        const doc = makeDocument(['rq get(unknown_var);']);
        const position = new vscode.Position(0, 7);

        (doc.getWordRangeAtPosition as jest.Mock).mockReturnValue({ start: position, end: position });
        (doc.getText as jest.Mock).mockReturnValue('unknown_var');
        (cliService.varRefs as jest.Mock).mockRejectedValue(new Error('not found'));

        const result = await provideReferences(doc, position);

        expect(result).toEqual([]);
    });

    test('returns empty array when no word at cursor', async () => {
        const doc = makeDocument(['rq get();']);
        const position = new vscode.Position(0, 8);

        (doc.getWordRangeAtPosition as jest.Mock).mockReturnValue(undefined);

        const result = await provideReferences(doc, position);

        expect(result).toEqual([]);
        expect(cliService.varRefs).not.toHaveBeenCalled();
    });

    test('uses undefined sourceDirectory when no workspace folders', async () => {
        (vscode.workspace as any).workspaceFolders = undefined;

        const doc = makeDocument(['rq get(my_var);']);
        const position = new vscode.Position(0, 7);

        (doc.getWordRangeAtPosition as jest.Mock).mockReturnValue({ start: position, end: position });
        (doc.getText as jest.Mock).mockReturnValue('my_var');
        (cliService.varRefs as jest.Mock).mockResolvedValue([]);

        await provideReferences(doc, position);

        expect(cliService.varRefs).toHaveBeenCalledWith('my_var', undefined);
    });
});

describe('endpoint template references', () => {
    test('calls epRefs when cursor is on the parent name in ep template line', async () => {
        const line = 'ep child<base>(url: "http://localhost");';
        const doc = makeDocument([line]);
        const parentStart = line.indexOf('base');
        const position = new vscode.Position(0, parentStart + 1);

        (cliService.epRefs as jest.Mock).mockResolvedValue([
            { file: '/workspace/a.rq', line: 0, character: 9 }
        ]);

        const result = await provideReferences(doc, position);

        expect(cliService.epRefs).toHaveBeenCalledWith('base', '/workspace');
        expect(cliService.varRefs).not.toHaveBeenCalled();
        expect(result).toHaveLength(1);
        expect(result[0]).toBeInstanceOf(vscode.Location);
        expect((result[0] as vscode.Location).range.start).toEqual(new vscode.Position(0, 9));
    });

    test('returns empty array when epRefs throws', async () => {
        const line = 'ep child<base>(url: "http://localhost");';
        const doc = makeDocument([line]);
        const parentStart = line.indexOf('base');
        const position = new vscode.Position(0, parentStart + 1);

        (cliService.epRefs as jest.Mock).mockRejectedValue(new Error('not found'));

        const result = await provideReferences(doc, position);

        expect(result).toEqual([]);
    });

    test('does not call epRefs when cursor is outside the parent name', async () => {
        const line = 'ep child<base>(url: "http://localhost");';
        const doc = makeDocument([line]);
        const position = new vscode.Position(0, 2);

        (doc.getWordRangeAtPosition as jest.Mock).mockReturnValue({ start: position, end: position });
        (doc.getText as jest.Mock).mockReturnValue('ep');
        (cliService.varRefs as jest.Mock).mockResolvedValue([]);

        await provideReferences(doc, position);

        expect(cliService.epRefs).not.toHaveBeenCalled();
    });
});
