jest.mock('../../src/cliService');

import * as vscode from 'vscode';
import * as cliService from '../../src/cliService';
import { setEnvironmentProvider } from '../../src/language/definitionProvider';

function makeDocument(lines: string[], uri: any = { fsPath: '/test/file.rq' }) {
    return {
        uri,
        lineCount: lines.length,
        lineAt: (i: number | vscode.Position) => {
            const idx = typeof i === 'number' ? i : (i as vscode.Position).line;
            return { text: lines[idx] };
        },
        getWordRangeAtPosition: jest.fn(),
        getText: jest.fn()
    };
}

let provideDefinition: Function;

beforeAll(() => {
    const calls = (vscode.languages.registerDefinitionProvider as jest.Mock).mock.calls;
    provideDefinition = calls[calls.length - 1][1].provideDefinition;
});

beforeEach(() => {
    jest.clearAllMocks();
    (vscode.workspace as any).workspaceFolders = [];
});

describe('when no environment is selected', () => {
    beforeEach(() => {
        setEnvironmentProvider({ getSelectedEnvironment: () => undefined });
    });

    test('resolves local let variable to correct line without calling CLI', async () => {
        const lines = [
            'import "_shared";',
            '',
            'let my_url = "http://example.com";',
            '',
            'rq get(my_url);'
        ];
        const doc = makeDocument(lines);
        const position = new vscode.Position(4, 7);

        (doc.getWordRangeAtPosition as jest.Mock).mockReturnValue({ start: position, end: position });
        (doc.getText as jest.Mock).mockReturnValue('my_url');

        const result = await provideDefinition(doc, position);

        expect(result).toBeInstanceOf(vscode.Location);
        expect((result as vscode.Location).uri).toBe(doc.uri);
        expect((result as vscode.Location).range.start).toEqual(new vscode.Position(2, 0));
        expect(cliService.showVariable).not.toHaveBeenCalled();
    });

    test('falls back to CLI for cross-file variables', async () => {
        const doc = makeDocument(['rq get(remote_var);']);
        const position = new vscode.Position(0, 7);

        (doc.getWordRangeAtPosition as jest.Mock).mockReturnValue({ start: position, end: position });
        (doc.getText as jest.Mock).mockReturnValue('remote_var');
        (cliService.showVariable as jest.Mock).mockResolvedValue({
            name: 'remote_var',
            file: '/other/file.rq',
            line: 5,
            character: 4,
            source: 'let'
        });
        (vscode.workspace as any).workspaceFolders = [{ uri: { fsPath: '/test' } }];

        const result = await provideDefinition(doc, position);

        expect(cliService.showVariable).toHaveBeenCalledWith('remote_var', '/test', undefined);
        expect(result).toBeInstanceOf(vscode.Location);
        expect((result as vscode.Location).range.start).toEqual(new vscode.Position(5, 4));
    });

    test('returns null when variable not found anywhere', async () => {
        const doc = makeDocument(['rq get(unknown_var);']);
        const position = new vscode.Position(0, 7);

        (doc.getWordRangeAtPosition as jest.Mock).mockReturnValue({ start: position, end: position });
        (doc.getText as jest.Mock).mockReturnValue('unknown_var');
        (cliService.showVariable as jest.Mock).mockRejectedValue(new Error('not found'));

        const result = await provideDefinition(doc, position);

        expect(result).toBeNull();
    });

    test('returns null when no word at position', async () => {
        const doc = makeDocument(['rq get();']);
        const position = new vscode.Position(0, 7);

        (doc.getWordRangeAtPosition as jest.Mock).mockReturnValue(undefined);

        const result = await provideDefinition(doc, position);

        expect(result).toBeNull();
        expect(cliService.showVariable).not.toHaveBeenCalled();
    });
});

describe('when environment is selected', () => {
    beforeEach(() => {
        setEnvironmentProvider({ getSelectedEnvironment: () => 'local' });
    });

    test('uses CLI and respects env precedence over local let', async () => {
        const lines = [
            'env local {',
            '    api_url: "http://localhost:8080",',
            '}',
            'let api_url = "http://prod.example.com";',
            '',
            'rq get(api_url);'
        ];
        const doc = makeDocument(lines);
        const position = new vscode.Position(5, 7);

        (doc.getWordRangeAtPosition as jest.Mock).mockReturnValue({ start: position, end: position });
        (doc.getText as jest.Mock).mockReturnValue('api_url');
        (cliService.showVariable as jest.Mock).mockResolvedValue({
            name: 'api_url',
            file: '/test/file.rq',
            line: 1,
            character: 4,
            source: 'env:local'
        });
        (vscode.workspace as any).workspaceFolders = [{ uri: { fsPath: '/test' } }];

        const result = await provideDefinition(doc, position);

        expect(cliService.showVariable).toHaveBeenCalledWith('api_url', '/test', 'local');
        expect(result).toBeInstanceOf(vscode.Location);
        expect((result as vscode.Location).range.start).toEqual(new vscode.Position(1, 4));
    });

    test('returns null when CLI fails', async () => {
        const doc = makeDocument(['rq get(api_url);']);
        const position = new vscode.Position(0, 7);

        (doc.getWordRangeAtPosition as jest.Mock).mockReturnValue({ start: position, end: position });
        (doc.getText as jest.Mock).mockReturnValue('api_url');
        (cliService.showVariable as jest.Mock).mockRejectedValue(new Error('not found'));

        const result = await provideDefinition(doc, position);

        expect(result).toBeNull();
    });
});
