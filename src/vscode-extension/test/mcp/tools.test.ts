import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

const mockLint = jest.fn();
const mockCheck = jest.fn();
const mockListRequests = jest.fn();

jest.mock('../../src/wasm/rq_wasm', () => ({
    lint: mockLint,
    check: mockCheck,
    list_requests: mockListRequests
}), { virtual: true });

import { Diagnostic, lintRq, validateRq, listRequests, draftPath, workspaceFor } from '../../src/mcp/tools';

let workspace: string;

beforeAll(() => {
    workspace = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'rq-mcp-tools-')));
    fs.writeFileSync(path.join(workspace, 'widgets.rq'), 'ep widgets("http://x/widgets") {\n    rq list();\n}\n');
});

afterAll(() => fs.rmSync(workspace, { recursive: true, force: true }));

beforeEach(() => jest.clearAllMocks());

describe('draftPath', () => {
    it('places a bare filename inside the workspace', () => {
        expect(draftPath('/repo', 'users.rq')).toBe('/repo/users.rq');
    });

    it('keeps an absolute path as given', () => {
        expect(draftPath('/repo', '/elsewhere/users.rq')).toBe('/elsewhere/users.rq');
    });

    it('falls back to draft.rq when no filename is supplied', () => {
        expect(draftPath('/repo', undefined)).toBe('/repo/draft.rq');
    });
});

describe('workspaceFor', () => {
    it('prefers the explicit workspace path', async () => {
        await expect(workspaceFor('/repo', '/other/users.rq')).resolves.toBe('/repo');
    });

    it('falls back to the working directory', async () => {
        await expect(workspaceFor(undefined, undefined)).resolves.toBe(process.cwd().replace(/\\/g, '/'));
    });

    it('derives the parent directory of an absolute draft that exists on disk', async () => {
        const target = await workspaceFor(undefined, path.join(workspace, 'widgets.rq'));
        expect(target).toBe(workspace.replace(/\\/g, '/'));
    });

    it('derives the parent directory of an absolute draft that is not on disk yet', async () => {
        const target = await workspaceFor(undefined, path.join(workspace, 'unsaved.rq'));
        expect(target).toBe(workspace.replace(/\\/g, '/'));
    });

    it('keeps an absolute path that is itself a directory', async () => {
        await expect(workspaceFor(undefined, workspace)).resolves.toBe(workspace.replace(/\\/g, '/'));
    });
});

describe('validateRq for an unsaved draft', () => {
    it('scans the sibling files of the draft so imports can resolve', async () => {
        mockCheck.mockReturnValue(JSON.stringify({ errors: [] }));

        await validateRq({ source: 'import "widgets";\n', path: path.join(workspace, 'unsaved.rq') });

        const [filesJson] = mockCheck.mock.calls[0];
        const files = JSON.parse(filesJson) as Record<string, string>;
        expect(Object.keys(files).sort()).toEqual(
            [`${workspace}/unsaved.rq`, `${workspace}/widgets.rq`].map(f => f.replace(/\\/g, '/')).sort()
        );
    });
});

describe('lintRq', () => {
    it('reaches the wasm through the shared host rather than its own loader', async () => {
        mockLint.mockReturnValue(JSON.stringify({ ok: true, diagnostics: [] }));

        await lintRq({ source: 'rq list("http://x");\n', path: 'users.rq', workspace_path: workspace });

        expect(mockLint).toHaveBeenCalledTimes(1);
    });

    it('passes the unsaved draft to the linter alongside the workspace files', async () => {
        mockLint.mockReturnValue(JSON.stringify({ ok: true, diagnostics: [] }));
        const source = 'rq list("http://x");\n';

        await lintRq({ source, path: 'users.rq', workspace_path: workspace });

        const [filesJson, passedSource, draft] = mockLint.mock.calls[0];
        const files = JSON.parse(filesJson);
        expect(passedSource).toBe(source);
        expect(draft).toBe(`${workspace}/users.rq`);
        expect(files[draft]).toBe(source);
        expect(files[`${workspace}/widgets.rq`]).toContain('ep widgets');
    });

    it('returns the linter result unchanged', async () => {
        const diagnostics = [{ severity: 'error', rule: 'empty_url_string', message: 'x', line: 1, column: 1 }];
        mockLint.mockReturnValue(JSON.stringify({ ok: false, diagnostics }));

        const target = await lintRq({ source: 'rq list("");\n', path: 'users.rq', workspace_path: workspace });

        expect(target).toEqual({ ok: false, diagnostics });
    });
});

describe('validateRq', () => {
    it('maps parser errors into diagnostics and reports not ok', async () => {
        mockCheck.mockReturnValue(JSON.stringify({ errors: [{ file: 'users.rq', line: 2, column: 3, message: 'boom' }] }));

        const target = await validateRq({ source: 'rq(', path: 'users.rq', workspace_path: workspace });

        expect(target.ok).toBe(false);
        expect(target.diagnostics[0]).toMatchObject({ severity: 'error', message: 'boom', line: 2 });
    });

    it('is ok when the parser reports nothing', async () => {
        mockCheck.mockReturnValue(JSON.stringify({ errors: [] }));

        await expect(validateRq({ source: 'rq list("http://x");', workspace_path: workspace }))
            .resolves.toEqual({ ok: true, diagnostics: [] });
    });

    it('forwards the requested environment', async () => {
        mockCheck.mockReturnValue(JSON.stringify({ errors: [] }));

        await validateRq({ source: 'x', path: 'users.rq', workspace_path: workspace, env: 'local' });

        expect(mockCheck.mock.calls[0][3]).toBe('local');
    });
});

describe('listRequests', () => {
    it('returns the requests the wasm reports', async () => {
        mockListRequests.mockReturnValue(JSON.stringify({ requests: [{ name: 'widgets/list' }], parse_errors: [] }));

        const target = await listRequests({ path: workspace }) as { requests: { name: string }[] };

        expect(target.requests[0].name).toBe('widgets/list');
    });

    it('reports the parse errors the wasm found instead of an empty array', async () => {
        mockListRequests.mockReturnValue(JSON.stringify({
            requests: [{ name: 'widgets/list' }],
            parse_errors: [{ severity: 'error', message: 'Expected \')\'', line: 2, column: 5, file: 'broken.rq' }]
        }));

        const target = await listRequests({ path: workspace }) as { parse_errors: Diagnostic[] };

        expect(target.parse_errors).toHaveLength(1);
        expect(target.parse_errors[0].message).toBe('Expected \')\'');
        expect(target.parse_errors[0].file).toBe('broken.rq');
    });

    it('defaults to an empty list when the wasm reports no parse errors', async () => {
        mockListRequests.mockReturnValue(JSON.stringify({ requests: [] }));

        const target = await listRequests({ path: workspace }) as { parse_errors: Diagnostic[] };

        expect(target.parse_errors).toEqual([]);
    });
});
