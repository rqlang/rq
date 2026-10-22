import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

const mockLint = jest.fn();

jest.mock('../src/wasm/rq_wasm', () => ({ lint: mockLint }), { virtual: true });

import { lintSource, lintSources } from '../src/rqClient';

let workspace: string;

beforeAll(() => {
    workspace = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'rq-lint-')));
    fs.writeFileSync(path.join(workspace, 'widgets.rq'), 'ep widgets("http://x/widgets") {\n    rq list();\n}\n');
});

afterAll(() => fs.rmSync(workspace, { recursive: true, force: true }));

beforeEach(() => jest.clearAllMocks());

function lintedAs(results: Record<string, unknown>): void {
    mockLint.mockReturnValue(JSON.stringify(results));
}

describe('rqClient.lintSource', () => {
    it('makes lint a first-class extension capability', async () => {
        const draft = path.join(workspace, 'users.rq');
        lintedAs({ [draft]: { ok: false, diagnostics: [{ rule: 'empty_url_string' }] } });

        const target = await lintSource('rq list("");\n', draft, workspace);

        expect(target.ok).toBe(false);
        expect(target.diagnostics[0].rule).toBe('empty_url_string');
    });
});

describe('rqClient.lintSources', () => {
    it('lints the whole batch in a single call so the workspace is read once', async () => {
        lintedAs({});

        await lintSources([
            { path: path.join(workspace, 'users.rq'), source: 'rq a("http://x");\n' },
            { path: path.join(workspace, 'orders.rq'), source: 'rq b("http://x");\n' }
        ], workspace);

        expect(mockLint).toHaveBeenCalledTimes(1);
    });

    it('sends every unsaved draft and leaves the saved files to the linter', async () => {
        lintedAs({});

        await lintSources([
            { path: path.join(workspace, 'users.rq'), source: 'rq a("http://x");\n' },
            { path: path.join(workspace, 'orders.rq'), source: 'rq b("http://x");\n' }
        ], workspace);

        const files = JSON.parse(mockLint.mock.calls[0][0]);
        expect(files[`${workspace}/users.rq`]).toContain('rq a(');
        expect(files[`${workspace}/orders.rq`]).toContain('rq b(');
        expect(Object.keys(files)).toHaveLength(2);
    });

    it('passes the workspace so the linter reads the files around the drafts', async () => {
        lintedAs({});

        await lintSources([{ path: path.join(workspace, 'users.rq'), source: 'x' }], workspace);

        expect(mockLint.mock.calls[0][1]).toBe(workspace.replace(/\\/g, '/'));
    });

    it('hands each draft the result the linter returned for it', async () => {
        const users = `${workspace}/users.rq`;
        const orders = `${workspace}/orders.rq`;
        lintedAs({
            [users]: { ok: false, diagnostics: [{ rule: 'empty_url_string' }] },
            [orders]: { ok: true, diagnostics: [] }
        });

        const target = await lintSources([{ path: users, source: 'x' }, { path: orders, source: 'y' }], workspace);

        expect(target.get(users)?.ok).toBe(false);
        expect(target.get(orders)?.ok).toBe(true);
    });

    it('keys the results by normalized path', async () => {
        lintedAs({});

        const target = await lintSources([{ path: path.join(workspace, 'users.rq'), source: 'x' }], workspace);

        expect(target.has(`${workspace}/users.rq`)).toBe(true);
    });

    it('does nothing when there is nothing to lint', async () => {
        const target = await lintSources([], workspace);

        expect(target.size).toBe(0);
        expect(mockLint).not.toHaveBeenCalled();
    });
});
