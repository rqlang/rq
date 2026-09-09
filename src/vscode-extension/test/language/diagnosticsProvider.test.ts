import * as vscode from 'vscode';
import * as rqClient from '../../src/rqClient';
import { DiagnosticsProvider } from '../../src/language/diagnosticsProvider';

jest.mock('../../src/rqClient');

const mockedClient = rqClient as jest.Mocked<typeof rqClient>;
const workspace = (vscode as any).workspace;

const FOLDER = { uri: { fsPath: '/repo' } };

function collection() {
    const entries = new Map<string, vscode.Diagnostic[]>();
    return {
        entries,
        set: jest.fn((uri: any, diagnostics: vscode.Diagnostic[]) => entries.set(uri.fsPath, diagnostics)),
        delete: jest.fn((uri: any) => entries.delete(uri.fsPath)),
        [Symbol.iterator]: function* () { yield* [] as any; }
    } as any;
}

function documentAt(fsPath: string, text: string) {
    return { languageId: 'rq', isDirty: false, uri: { fsPath }, getText: () => text };
}

beforeEach(() => {
    jest.clearAllMocks();
    workspace.workspaceFolders = [FOLDER];
    workspace.getWorkspaceFolder = jest.fn().mockReturnValue(FOLDER);
    workspace.getConfiguration = jest.fn().mockReturnValue({ get: (_k: string, d: boolean) => d });
    workspace.textDocuments = [documentAt('/repo/users.rq', 'rq list("");\n')];
    mockedClient.checkFolder.mockResolvedValue({ errors: [] } as any);
    mockedClient.lintSources.mockResolvedValue(new Map());
});

describe('DiagnosticsProvider lint integration', () => {
    it('surfaces lint findings as warnings, not errors', async () => {
        mockedClient.lintSources.mockResolvedValue(new Map([
            ['/repo/users.rq', { ok: false, diagnostics: [
                { severity: 'error', rule: 'empty_url_string', message: 'empty url', line: 1, column: 9 }
            ] }]
        ]) as any);
        const target = new DiagnosticsProvider(collection());

        await (target as any).validateFolder(FOLDER);

        const [, diagnostics] = ((target as any).diagnosticCollection.set as jest.Mock).mock.calls[0];
        expect(diagnostics[0].severity).toBe(vscode.DiagnosticSeverity.Warning);
    });

    it('tags lint findings with their rule so they can be told apart from syntax errors', async () => {
        mockedClient.lintSources.mockResolvedValue(new Map([
            ['/repo/users.rq', { ok: false, diagnostics: [
                { severity: 'error', rule: 'empty_url_string', message: 'empty url', line: 1, column: 9 }
            ] }]
        ]) as any);
        const target = new DiagnosticsProvider(collection());

        await (target as any).validateFolder(FOLDER);

        const [, diagnostics] = ((target as any).diagnosticCollection.set as jest.Mock).mock.calls[0];
        expect(diagnostics[0].source).toBe('rq lint');
        expect(diagnostics[0].code).toBe('empty_url_string');
    });

    it('appends the suggested fix to the message', async () => {
        mockedClient.lintSources.mockResolvedValue(new Map([
            ['/repo/users.rq', { ok: false, diagnostics: [
                { severity: 'error', rule: 'empty_url_string', message: 'empty url', line: 1, column: 9, suggested_fix: 'write rq list();' }
            ] }]
        ]) as any);
        const target = new DiagnosticsProvider(collection());

        await (target as any).validateFolder(FOLDER);

        const [, diagnostics] = ((target as any).diagnosticCollection.set as jest.Mock).mock.calls[0];
        expect(diagnostics[0].message).toContain('empty url');
        expect(diagnostics[0].message).toContain('Suggested fix: write rq list();');
    });

    it('does not lint when the setting is turned off', async () => {
        workspace.getConfiguration = jest.fn().mockReturnValue({ get: () => false });
        const target = new DiagnosticsProvider(collection());

        await (target as any).validateFolder(FOLDER);

        expect(mockedClient.lintSources).not.toHaveBeenCalled();
    });

    it('keeps syntax errors visible when the linter fails', async () => {
        mockedClient.lintSources.mockRejectedValue(new Error('wasm exploded'));
        mockedClient.checkFolder.mockResolvedValue({
            errors: [{ file: '/repo/users.rq', line: 3, column: 1, message: 'boom' }]
        } as any);
        const target = new DiagnosticsProvider(collection());

        await (target as any).validateFolder(FOLDER);

        const [, diagnostics] = ((target as any).diagnosticCollection.set as jest.Mock).mock.calls[0];
        expect(diagnostics).toHaveLength(1);
        expect(diagnostics[0].severity).toBe(vscode.DiagnosticSeverity.Error);
    });

    it('reports syntax errors and lint findings on the same file together', async () => {
        mockedClient.checkFolder.mockResolvedValue({
            errors: [{ file: '/repo/users.rq', line: 3, column: 1, message: 'boom' }]
        } as any);
        mockedClient.lintSources.mockResolvedValue(new Map([
            ['/repo/users.rq', { ok: false, diagnostics: [
                { severity: 'error', rule: 'empty_url_string', message: 'empty url', line: 1, column: 9 }
            ] }]
        ]) as any);
        const target = new DiagnosticsProvider(collection());

        await (target as any).validateFolder(FOLDER);

        const [, diagnostics] = ((target as any).diagnosticCollection.set as jest.Mock).mock.calls[0];
        expect(diagnostics).toHaveLength(2);
        expect(diagnostics.map((d: any) => d.severity).sort()).toEqual(
            [vscode.DiagnosticSeverity.Error, vscode.DiagnosticSeverity.Warning].sort()
        );
    });
});


describe('DiagnosticsProvider lint setting changes', () => {
    function changeHandler(): (event: any) => void {
        return workspace.onDidChangeConfiguration.mock.calls[0][0];
    }

    it('revalidates when rq.lint.enabled changes so stale warnings clear immediately', () => {
        const target = new DiagnosticsProvider(collection());
        const revalidate = jest.spyOn(target, 'validateAllFolders').mockImplementation(() => {});

        changeHandler()({ affectsConfiguration: (setting: string) => setting === 'rq.lint.enabled' });

        expect(revalidate).toHaveBeenCalled();
        target.dispose();
    });

    it('ignores changes to unrelated settings', () => {
        const target = new DiagnosticsProvider(collection());
        const revalidate = jest.spyOn(target, 'validateAllFolders').mockImplementation(() => {});

        changeHandler()({ affectsConfiguration: () => false });

        expect(revalidate).not.toHaveBeenCalled();
        target.dispose();
    });

    it('disposes its configuration listener', () => {
        const dispose = jest.fn();
        workspace.onDidChangeConfiguration.mockReturnValueOnce({ dispose });
        const target = new DiagnosticsProvider(collection());

        target.dispose();

        expect(dispose).toHaveBeenCalled();
    });
});
