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

describe('rqClient.lintSource', () => {
    it('makes lint a first-class extension capability', async () => {
        mockLint.mockReturnValue(JSON.stringify({ ok: false, diagnostics: [{ rule: 'empty_url_string' }] }));

        const target = await lintSource('rq list("");\n', path.join(workspace, 'users.rq'), workspace);

        expect(target.ok).toBe(false);
        expect(target.diagnostics[0].rule).toBe('empty_url_string');
    });
});

describe('rqClient.lintSources', () => {
    it('builds the workspace file map once for the whole batch', async () => {
        mockLint.mockReturnValue(JSON.stringify({ ok: true, diagnostics: [] }));

        await lintSources([
            { path: path.join(workspace, 'users.rq'), source: 'rq a("http://x");\n' },
            { path: path.join(workspace, 'orders.rq'), source: 'rq b("http://x");\n' }
        ], workspace);

        expect(mockLint).toHaveBeenCalledTimes(2);
        const [firstMap] = mockLint.mock.calls[0];
        const [secondMap] = mockLint.mock.calls[1];
        expect(firstMap).toBe(secondMap);
    });

    it('lets every draft see the unsaved content of its siblings', async () => {
        mockLint.mockReturnValue(JSON.stringify({ ok: true, diagnostics: [] }));

        await lintSources([
            { path: path.join(workspace, 'users.rq'), source: 'rq a("http://x");\n' },
            { path: path.join(workspace, 'orders.rq'), source: 'rq b("http://x");\n' }
        ], workspace);

        const files = JSON.parse(mockLint.mock.calls[0][0]);
        expect(files[`${workspace}/users.rq`]).toContain('rq a(');
        expect(files[`${workspace}/orders.rq`]).toContain('rq b(');
        expect(files[`${workspace}/widgets.rq`]).toContain('ep widgets');
    });

    it('keys the results by normalized path', async () => {
        mockLint.mockReturnValue(JSON.stringify({ ok: true, diagnostics: [] }));

        const target = await lintSources([{ path: path.join(workspace, 'users.rq'), source: 'x' }], workspace);

        expect(target.has(`${workspace}/users.rq`)).toBe(true);
    });

    it('does nothing when there is nothing to lint', async () => {
        const target = await lintSources([], workspace);

        expect(target.size).toBe(0);
        expect(mockLint).not.toHaveBeenCalled();
    });
});
