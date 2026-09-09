import * as fs from 'fs';
import * as vscode from 'vscode';
import { mcpServerEntry, registerMcpServer } from '../src/mcpServer';

jest.mock('fs');

const mockedFs = fs as jest.Mocked<typeof fs>;
const lm = (vscode as any).lm;
const workspace = (vscode as any).workspace;

function contextWith(extensionPath: string): any {
    return {
        extensionPath,
        extension: { packageJSON: { version: '1.2.3' } },
        subscriptions: [] as any[]
    };
}

describe('mcpServerEntry', () => {
    it('resolves the bundled JavaScript entry point', () => {
        expect(mcpServerEntry('/ext')).toBe('/ext/out/mcpServer.js');
    });
});

describe('registerMcpServer', () => {
    beforeEach(() => {
        jest.clearAllMocks();
        workspace.workspaceFolders = [];
    });

    afterEach(() => jest.restoreAllMocks());

    it('runs the server on the editor own Node rather than a bundled executable', () => {
        mockedFs.existsSync.mockReturnValue(true);

        registerMcpServer(contextWith('/ext'));

        const provider = lm.registerMcpServerDefinitionProvider.mock.calls[0][1];
        const [definition] = provider.provideMcpServerDefinitions();
        expect(definition.command).toBe(process.execPath);
        expect(definition.args).toEqual([mcpServerEntry('/ext')]);
    });

    it('registers under the id declared in the manifest contribution', () => {
        mockedFs.existsSync.mockReturnValue(true);

        registerMcpServer(contextWith('/ext'));

        expect(lm.registerMcpServerDefinitionProvider).toHaveBeenCalledWith('rq-lang.rq-mcp', expect.anything());
    });

    it('runs the editor binary in node mode, since process.execPath is Electron in the host', () => {
        mockedFs.existsSync.mockReturnValue(true);

        registerMcpServer(contextWith('/ext'));

        const provider = lm.registerMcpServerDefinitionProvider.mock.calls[0][1];
        expect(provider.provideMcpServerDefinitions()[0].env).toMatchObject({ ELECTRON_RUN_AS_NODE: '1' });
    });

    it('reports the extension version so the editor refreshes tools on upgrade', () => {
        mockedFs.existsSync.mockReturnValue(true);

        registerMcpServer(contextWith('/ext'));

        const provider = lm.registerMcpServerDefinitionProvider.mock.calls[0][1];
        expect(provider.provideMcpServerDefinitions()[0].version).toBe('1.2.3');
    });

    it('runs the server in the workspace folder so relative paths resolve', () => {
        mockedFs.existsSync.mockReturnValue(true);
        workspace.workspaceFolders = [{ uri: { fsPath: '/repo' } }];

        registerMcpServer(contextWith('/ext'));

        const provider = lm.registerMcpServerDefinitionProvider.mock.calls[0][1];
        expect(provider.provideMcpServerDefinitions()[0].cwd).toEqual({ fsPath: '/repo' });
    });

    it('does not register when the bundled entry point is missing', () => {
        mockedFs.existsSync.mockReturnValue(false);
        jest.spyOn(console, 'error').mockImplementation(() => undefined);

        const target = contextWith('/ext');
        registerMcpServer(target);

        expect(lm.registerMcpServerDefinitionProvider).not.toHaveBeenCalled();
        expect(target.subscriptions).toHaveLength(0);
    });
});
