jest.mock('../../src/rqClient');

import * as vscode from 'vscode';
import * as cliService from '../../src/rqClient';
import '../../src/language/renameProvider';

function makeDocument(lines: string[], uri: any = { fsPath: '/test/file.rq' }) {
    return {
        uri,
        lineAt: (i: number | vscode.Position) => {
            const idx = typeof i === 'number' ? i : (i as vscode.Position).line;
            return { text: lines[idx] };
        },
        getWordRangeAtPosition: jest.fn(),
        getText: jest.fn()
    };
}

let prepareRename: Function;
let provideRenameEdits: Function;

beforeAll(() => {
    const calls = (vscode.languages.registerRenameProvider as jest.Mock).mock.calls;
    const provider = calls[calls.length - 1][1];
    prepareRename = provider.prepareRename;
    provideRenameEdits = provider.provideRenameEdits;
});

beforeEach(() => {
    jest.clearAllMocks();
    (vscode.workspace as any).workspaceFolders = [{ uri: { fsPath: '/workspace' } }];
});

describe('prepareRename', () => {
    test('returns range and placeholder for variable', () => {
        const doc = makeDocument(['rq get("{{base_url}}/v1");']);
        const position = new vscode.Position(0, 10);

        (doc.getWordRangeAtPosition as jest.Mock).mockReturnValue(
            new vscode.Range(new vscode.Position(0, 8), new vscode.Position(0, 16))
        );
        (doc.getText as jest.Mock).mockReturnValue('base_url');

        const result = prepareRename(doc, position);

        expect(result.placeholder).toBe('base_url');
    });

    test('returns range and placeholder for ep parent', () => {
        const line = 'ep child<base>(url: "http://localhost");';
        const doc = makeDocument([line]);
        const parentStart = line.indexOf('base');
        const position = new vscode.Position(0, parentStart + 1);

        const result = prepareRename(doc, position);

        expect(result.placeholder).toBe('base');
    });

    test('returns range and placeholder for rq statement name', () => {
        const doc = makeDocument(['rq my_request("https://example.com");']);
        const position = new vscode.Position(0, 5);

        const result = prepareRename(doc, position);

        expect(result.placeholder).toBe('my_request');
    });

    test('throws when no word at cursor', () => {
        const doc = makeDocument(['rq get();']);
        const position = new vscode.Position(0, 8);

        (doc.getWordRangeAtPosition as jest.Mock).mockReturnValue(undefined);

        expect(() => prepareRename(doc, position)).toThrow('No renameable symbol');
    });

    test('throws when cursor is on a keyword', () => {
        const doc = makeDocument(['rq my_request();']);
        const position = new vscode.Position(0, 1);

        (doc.getWordRangeAtPosition as jest.Mock).mockReturnValue(
            new vscode.Range(new vscode.Position(0, 0), new vscode.Position(0, 2))
        );
        (doc.getText as jest.Mock).mockReturnValue('rq');

        expect(() => prepareRename(doc, position)).toThrow('No renameable symbol');
    });
});

describe('variable rename', () => {
    test('calls varRefs and returns WorkspaceEdit with edits for each ref', async () => {
        const doc = makeDocument(['rq get("{{base_url}}/v1");']);
        const position = new vscode.Position(0, 10);

        (doc.getWordRangeAtPosition as jest.Mock).mockReturnValue(
            new vscode.Range(new vscode.Position(0, 8), new vscode.Position(0, 16))
        );
        (doc.getText as jest.Mock).mockReturnValue('base_url');
        (cliService.varRefs as jest.Mock).mockResolvedValue([
            { file: '/workspace/a.rq', line: 0, character: 4 },
            { file: '/workspace/b.rq', line: 2, character: 9 }
        ]);

        const result = await provideRenameEdits(doc, position, 'new_url');

        expect(cliService.varRefs).toHaveBeenCalledWith('base_url', '/workspace');
        expect(result).toBeInstanceOf(vscode.WorkspaceEdit);
        expect((result as any).edits).toHaveLength(2);
        expect((result as any).edits[0].uri.fsPath).toBe('/workspace/a.rq');
        expect((result as any).edits[0].newText).toBe('new_url');
        expect((result as any).edits[1].uri.fsPath).toBe('/workspace/b.rq');
    });

    test('returns undefined when varRefs throws', async () => {
        const doc = makeDocument(['rq get("{{base_url}}/v1");']);
        const position = new vscode.Position(0, 10);

        (doc.getWordRangeAtPosition as jest.Mock).mockReturnValue(
            new vscode.Range(new vscode.Position(0, 8), new vscode.Position(0, 16))
        );
        (doc.getText as jest.Mock).mockReturnValue('base_url');
        (cliService.varRefs as jest.Mock).mockRejectedValue(new Error('not found'));

        const result = await provideRenameEdits(doc, position, 'new_url');

        expect(result).toBeUndefined();
    });

    test('returns undefined when varRefs returns empty array', async () => {
        const doc = makeDocument(['rq get("{{base_url}}/v1");']);
        const position = new vscode.Position(0, 10);

        (doc.getWordRangeAtPosition as jest.Mock).mockReturnValue(
            new vscode.Range(new vscode.Position(0, 8), new vscode.Position(0, 16))
        );
        (doc.getText as jest.Mock).mockReturnValue('base_url');
        (cliService.varRefs as jest.Mock).mockResolvedValue([]);

        const result = await provideRenameEdits(doc, position, 'new_url');

        expect(result).toBeUndefined();
    });

    test('throws for invalid new name', async () => {
        const doc = makeDocument(['let base_url = "http://example.com";']);
        const position = new vscode.Position(0, 6);

        (doc.getWordRangeAtPosition as jest.Mock).mockReturnValue(
            new vscode.Range(new vscode.Position(0, 4), new vscode.Position(0, 12))
        );
        (doc.getText as jest.Mock).mockReturnValue('base_url');

        await expect(provideRenameEdits(doc, position, 'invalid name!')).rejects.toThrow('Invalid name');
    });
});

describe('endpoint rename', () => {
    test('from parent reference: renames definition + all refs via epRefs', async () => {
        const line = 'ep child<base>(url: "http://localhost");';
        const doc = makeDocument([line]);
        const parentStart = line.indexOf('base');
        const position = new vscode.Position(0, parentStart + 1);

        (cliService.epRefs as jest.Mock).mockResolvedValue([
            { file: '/workspace/base.rq', line: 0, character: 3 },
            { file: '/workspace/child.rq', line: 0, character: 9 }
        ]);

        const result = await provideRenameEdits(doc, position, 'new_base');

        expect(cliService.epRefs).toHaveBeenCalledWith('base', '/workspace');
        expect(cliService.showEndpoint).not.toHaveBeenCalled();
        expect(cliService.varRefs).not.toHaveBeenCalled();
        expect(result).toBeInstanceOf(vscode.WorkspaceEdit);
        expect((result as any).edits).toHaveLength(2);
        expect((result as any).edits[0].uri.fsPath).toBe('/workspace/base.rq');
        expect((result as any).edits[0].newText).toBe('new_base');
        expect((result as any).edits[1].uri.fsPath).toBe('/workspace/child.rq');
        expect((result as any).edits[1].newText).toBe('new_base');
    });

    test('from definition line: renames definition + all refs via epRefs', async () => {
        const line = 'ep base(url: "http://localhost");';
        const doc = makeDocument([line]);
        const nameStart = line.indexOf('base');
        const position = new vscode.Position(0, nameStart + 1);

        (cliService.epRefs as jest.Mock).mockResolvedValue([
            { file: '/workspace/base.rq', line: 0, character: 3 },
            { file: '/workspace/child.rq', line: 0, character: 9 }
        ]);

        const result = await provideRenameEdits(doc, position, 'new_base');

        expect(cliService.epRefs).toHaveBeenCalledWith('base', '/workspace');
        expect(cliService.showEndpoint).not.toHaveBeenCalled();
        expect((result as any).edits).toHaveLength(2);
    });

    test('returns undefined when epRefs throws', async () => {
        const line = 'ep child<base>(url: "http://localhost");';
        const doc = makeDocument([line]);
        const parentStart = line.indexOf('base');
        const position = new vscode.Position(0, parentStart + 1);

        (cliService.epRefs as jest.Mock).mockRejectedValue(new Error('not found'));

        const result = await provideRenameEdits(doc, position, 'new_base');

        expect(result).toBeUndefined();
    });

    test('returns undefined when epRefs returns empty array', async () => {
        const line = 'ep child<base>(url: "http://localhost");';
        const doc = makeDocument([line]);
        const parentStart = line.indexOf('base');
        const position = new vscode.Position(0, parentStart + 1);

        (cliService.epRefs as jest.Mock).mockResolvedValue([]);

        const result = await provideRenameEdits(doc, position, 'new_base');

        expect(result).toBeUndefined();
    });
});

describe('rq statement rename', () => {
    test('creates WorkspaceEdit with single edit at definition location', async () => {
        const doc = makeDocument(['rq my_request("https://example.com");']);
        const position = new vscode.Position(0, 5);

        const result = await provideRenameEdits(doc, position, 'renamed_request');

        expect(cliService.showRequestLocation).not.toHaveBeenCalled();
        expect(result).toBeInstanceOf(vscode.WorkspaceEdit);
        expect((result as any).edits).toHaveLength(1);
        expect((result as any).edits[0].uri.fsPath).toBe('/test/file.rq');
        expect((result as any).edits[0].newText).toBe('renamed_request');
    });

    test('returns undefined when no symbol at position', async () => {
        const doc = makeDocument(['rq get();']);
        const position = new vscode.Position(0, 8);

        (doc.getWordRangeAtPosition as jest.Mock).mockReturnValue(undefined);

        const result = await provideRenameEdits(doc, position, 'new_name');

        expect(result).toBeUndefined();
    });
});
