import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { after, before, describe, it } from 'node:test';
import { callJson, connect, tempWorkspace } from './client.mjs';

let client;

before(async () => {
    client = await connect();
});

after(async () => {
    await client.close();
});

describe('validate_rq', () => {
    it('is ok for a clean request', async () => {
        const target = await callJson(client, 'validate_rq', { source: 'rq basic("http://localhost:8080/get");\n' });
        assert.deepEqual(target, { ok: true, diagnostics: [] });
    });

    it('is ok for empty input', async () => {
        const target = await callJson(client, 'validate_rq', { source: '' });
        assert.equal(target.ok, true);
    });

    it('reports a syntax error with its line and column in the draft', async () => {
        const workspace = tempWorkspace();
        const target = await callJson(client, 'validate_rq', {
            source: 'rq basic("http://localhost:8080/get"\n',
            path: 'draft.rq',
            workspace_path: workspace
        });
        assert.equal(target.ok, false);
        assert.equal(target.diagnostics[0].severity, 'error');
        assert.ok(target.diagnostics[0].line >= 1);
        assert.ok(target.diagnostics[0].column >= 1);
        assert.ok(target.diagnostics[0].file.endsWith('draft.rq'), target.diagnostics[0].file);
    });

    it('blames the imported file that actually has the error', async () => {
        const workspace = tempWorkspace({ 'shared.rq': 'let broken = ;\n' });
        const target = await callJson(client, 'validate_rq', {
            source: 'import "shared";\n\nrq list("http://localhost:8080/users");\n',
            path: 'users.rq',
            workspace_path: workspace
        });
        assert.equal(target.ok, false);
        assert.ok(target.diagnostics[0].file.endsWith('shared.rq'), target.diagnostics[0].file);
    });

    it('resolves imports and the environment against the workspace', async () => {
        const workspace = tempWorkspace({ 'shared.rq': 'env local {\n    base_url: "http://localhost:8080",\n}\n' });
        const target = await callJson(client, 'validate_rq', {
            source: 'import "shared";\n\nrq list("{{base_url}}/users");\n',
            path: 'users.rq',
            workspace_path: workspace,
            env: 'local'
        });
        assert.deepEqual(target.diagnostics, []);
    });

    it('reports an import missing from the workspace', async () => {
        const workspace = tempWorkspace();
        const target = await callJson(client, 'validate_rq', {
            source: 'import "shared";\n\nrq list("http://localhost:8080/users");\n',
            path: 'users.rq',
            workspace_path: workspace
        });
        assert.equal(target.ok, false);
        assert.match(target.diagnostics[0].message, /shared/);
    });

    it('resolves an import next to a relative draft in a subdirectory', async () => {
        const workspace = tempWorkspace({ 'api/shared.rq': 'ep base(url: "http://localhost:8080");\n' });
        const target = await callJson(client, 'validate_rq', {
            source: 'import "shared";\n\nep users<base>("/users") {\n    rq list();\n}\n',
            path: 'api/users.rq',
            workspace_path: workspace
        });
        assert.deepEqual(target.diagnostics, []);
    });

    it('reports a missing workspace path as a tool error', async () => {
        const target = await client.callTool({
            name: 'validate_rq',
            arguments: { source: '', workspace_path: path.join(tempWorkspace(), 'missing') }
        });
        assert.equal(target.isError, true);
    });

    it('does not write the draft into the workspace', async () => {
        const workspace = tempWorkspace();
        await callJson(client, 'validate_rq', {
            source: 'rq basic("http://localhost:8080/get");\n',
            path: 'users.rq',
            workspace_path: workspace
        });
        assert.deepEqual(fs.readdirSync(workspace), []);
    });
});

describe('lint_rq', () => {
    it('returns rule-tagged diagnostics with a suggested fix', async () => {
        const target = await callJson(client, 'lint_rq', {
            source: 'import "/Users/me/shared";\nrq foo("http://x");\n',
            path: 'foo.rq',
            workspace_path: tempWorkspace()
        });
        const finding = target.diagnostics.find(d => d.rule === 'absolute_import_path');
        assert.equal(target.ok, false);
        assert.ok(finding.suggested_fix);
    });
});

describe('list_requests', () => {
    it('lists the request of a single file', async () => {
        const workspace = tempWorkspace({ 'greet.rq': 'rq greet("http://example.test/hi");\n' });
        const target = await callJson(client, 'list_requests', { path: path.join(workspace, 'greet.rq') });
        assert.deepEqual(target.requests.map(r => r.name), ['greet']);
    });

    it('lists every request under a directory', async () => {
        const workspace = tempWorkspace({
            'a.rq': 'rq alpha("http://example.test/a");\n',
            'b.rq': 'rq bravo("http://example.test/b");\n'
        });
        const target = await callJson(client, 'list_requests', { path: workspace });
        assert.deepEqual(target.requests.map(r => r.name).sort(), ['alpha', 'bravo']);
    });

    it('keeps listing the valid files when one is broken', async () => {
        const workspace = tempWorkspace({
            'good.rq': 'rq good("http://example.test/g");\n',
            'broken.rq': 'rq broken("http://x"\n'
        });
        const target = await callJson(client, 'list_requests', { path: workspace });
        assert.deepEqual(target.requests.map(r => r.name), ['good']);
    });

    it('blames the broken file rather than the directory', async () => {
        const workspace = tempWorkspace({
            'good.rq': 'rq good("http://example.test/g");\n',
            'broken.rq': 'rq broken("http://x"\n'
        });
        const target = await callJson(client, 'list_requests', { path: workspace });
        assert.equal(target.parse_errors.length, 1);
        assert.ok(target.parse_errors[0].file.endsWith('broken.rq'), target.parse_errors[0].file);
    });

    it('defaults to the directory the server runs in', async () => {
        const workspace = tempWorkspace({ 'greet.rq': 'rq greet("http://example.test/hi");\n' });
        const scoped = await connect(workspace);
        const target = await callJson(scoped, 'list_requests', {});
        await scoped.close();
        assert.deepEqual(target.requests.map(r => r.name), ['greet']);
    });

    it('reports a missing path as a tool error', async () => {
        const target = await client.callTool({ name: 'list_requests', arguments: { path: path.join(tempWorkspace(), 'missing') } });
        assert.equal(target.isError, true);
    });
});
