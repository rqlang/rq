import assert from 'node:assert/strict';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { after, before, beforeEach, describe, it } from 'node:test';
import { callJson, connect, tempWorkspace } from './client.mjs';

let api;
let received;
let workspace;
let client;

function apiSource(port) {
    return [
        'env local {',
        `    base_url: "http://127.0.0.1:${port}",`,
        '}',
        '',
        '[confirm]',
        'env production {',
        `    base_url: "http://127.0.0.1:${port}",`,
        '}',
        '',
        'auth api_auth(auth_type.bearer) {',
        '    token: "secret-token",',
        '}',
        '',
        '[auth("api_auth")]',
        'ep users("{{base_url}}/users") {',
        '    rq list();',
        '',
        '    [confirm]',
        '    rq audit("audit");',
        '',
        '    rq post(body: ${"name": "ada"});',
        '',
        '    [required(id)]',
        '    rq get(id);',
        '}',
        '',
        'rq big("{{base_url}}/big");',
        '',
        'let stamp = random.guid();',
        '',
        '[method(POST)]',
        'rq stamped("{{base_url}}/stamped/{{stamp}}", body: ${});',
        '',
        'rq keyed("{{base_url}}/keyed?key={{api_key}}", $["X-Key": "{{api_key}}"]);',
        '',
        'rq slow("{{base_url}}/slow");',
        ''
    ].join('\n');
}

before(async () => {
    api = http.createServer((req, res) => {
        received.push(`${req.method} ${req.url}`);
        if (req.url === '/slow') {
            res.on('close', () => received.push('closed'));
            return;
        }
        res.setHeader('content-type', 'application/json');
        res.setHeader('set-cookie', 'session=abc');
        res.end(req.url === '/big' ? JSON.stringify({ data: 'x'.repeat(60_000) }) : JSON.stringify({ path: req.url }));
    });
    await new Promise(resolve => api.listen(0, '127.0.0.1', resolve));
    workspace = tempWorkspace({ 'users.rq': apiSource(api.address().port), '.env': 'API_KEY=k3y-s3cret\n' });
    client = await connect(workspace);
});

after(async () => {
    await client.close();
    await new Promise(resolve => api.close(resolve));
});

beforeEach(() => {
    received = [];
});

const file = () => path.join(workspace, 'users.rq');

describe('run_request', () => {
    it('sends an unmarked read without asking', async () => {
        const target = await callJson(client, 'run_request', { name: 'users.list', file: file(), env: 'local' });
        assert.deepEqual([target.status, target.response.status, JSON.parse(target.response.body)], ['sent', 200, { path: '/users' }]);
    });

    it('holds a read against an environment marked confirm until the user confirms', async () => {
        const target = await callJson(client, 'run_request', { name: 'users/list', file: file(), env: 'production' });
        assert.deepEqual([target.status, typeof target.confirmation_token, received], ['confirmation_required', 'string', []]);
    });

    it('holds a read marked confirm', async () => {
        const target = await callJson(client, 'run_request', { name: 'users/audit', file: file(), env: 'local' });
        assert.deepEqual([target.status, received], ['confirmation_required', []]);
    });

    it('holds an unmarked write', async () => {
        const target = await callJson(client, 'run_request', { name: 'users/post', file: file(), env: 'local' });
        assert.deepEqual([target.status, received], ['confirmation_required', []]);
    });

    it('sends a held request once it is called again with its confirmation token', async () => {
        const args = { name: 'users/post', file: file(), env: 'local' };
        const { confirmation_token } = await callJson(client, 'run_request', args);
        const target = await callJson(client, 'run_request', { ...args, confirmation_token });
        assert.deepEqual([target.status, received], ['sent', ['POST /users']]);
    });

    it('refuses a confirmation token a second time', async () => {
        const args = { name: 'users/post', file: file(), env: 'local' };
        const { confirmation_token } = await callJson(client, 'run_request', args);
        await callJson(client, 'run_request', { ...args, confirmation_token });
        const target = await client.callTool({ name: 'run_request', arguments: { ...args, confirmation_token } });
        assert.deepEqual([target.isError, received], [true, ['POST /users']]);
    });

    it('refuses a confirmation token issued for a different request', async () => {
        const { confirmation_token } = await callJson(client, 'run_request', { name: 'users/list', file: file(), env: 'production' });
        const target = await client.callTool({ name: 'run_request', arguments: { name: 'users/post', file: file(), env: 'local', confirmation_token } });
        assert.deepEqual([target.isError, received], [true, []]);
    });

    it('refuses a confirmation token for the same request with other variables', async () => {
        const args = { name: 'users/get', file: file(), env: 'production' };
        const { confirmation_token } = await callJson(client, 'run_request', { ...args, variables: { id: '1' } });
        const target = await client.callTool({ name: 'run_request', arguments: { ...args, variables: { id: '2' }, confirmation_token } });
        assert.deepEqual([target.isError, received], [true, []]);
    });

    it('sends a held request whose url changes on every call once it is confirmed', async () => {
        const args = { name: 'stamped', file: file(), env: 'local' };
        const { confirmation_token } = await callJson(client, 'run_request', args);
        const target = await callJson(client, 'run_request', { ...args, confirmation_token });
        assert.deepEqual([target.status, received.length], ['sent', 1]);
    });

    it('refuses a confirmation token once the request was edited', async () => {
        const orders = path.join(workspace, 'orders.rq');
        fs.writeFileSync(orders, '[method(POST)]\nrq create("http://127.0.0.1:1/orders", body: ${"total": 1});\n');
        const args = { name: 'create', file: orders };
        const { confirmation_token } = await callJson(client, 'run_request', args);
        fs.writeFileSync(orders, '[method(POST)]\nrq create("http://127.0.0.1:1/orders", body: ${"total": 1000});\n');
        const target = await client.callTool({ name: 'run_request', arguments: { ...args, confirmation_token } });
        fs.rmSync(orders);
        assert.equal(target.isError, true);
    });

    it('redacts the secrets in the url and headers it sent', async () => {
        const target = await callJson(client, 'run_request', { name: 'keyed', file: file(), env: 'local' });
        assert.deepEqual(
            [received, target.request.url.endsWith('?key=[redacted]'), target.request.headers['x-key'] ?? target.request.headers['X-Key']],
            [['GET /keyed?key=k3y-s3cret'], true, '[redacted]']
        );
    });

    it('stops sending once the tool call is cancelled', async () => {
        const cancellation = new AbortController();
        const call = client.callTool({ name: 'run_request', arguments: { name: 'slow', file: file(), env: 'local' } }, undefined, { signal: cancellation.signal });
        while (received.length === 0) {
            await new Promise(resolve => setTimeout(resolve, 10));
        }
        cancellation.abort();
        await assert.rejects(call);
        while (received.length < 2) {
            await new Promise(resolve => setTimeout(resolve, 10));
        }
        assert.deepEqual(received, ['GET /slow', 'closed']);
    });

    it('redacts the credentials it sent and received', async () => {
        const target = await callJson(client, 'run_request', { name: 'users/list', file: file(), env: 'local' });
        assert.deepEqual(
            [target.request.headers.Authorization ?? target.request.headers.authorization, target.response.headers['set-cookie']],
            ['[redacted]', '[redacted]']
        );
    });

    it('truncates a long response body', async () => {
        const target = await callJson(client, 'run_request', { name: 'big', file: file(), env: 'local' });
        assert.deepEqual([target.response.body.length, target.response.body_truncated], [50_000, true]);
    });

    it('reports a request that does not exist as a tool error', async () => {
        const target = await client.callTool({ name: 'run_request', arguments: { name: 'users/nope', file: file() } });
        assert.equal(target.isError, true);
    });
});

describe('run_request with a client that can ask the user', () => {
    let asking;
    let questions;

    before(async () => {
        asking = await connect(workspace, message => {
            questions.push(message);
            return { action: message.includes('POST') ? 'decline' : 'accept' };
        });
    });

    after(async () => {
        await asking.close();
    });

    beforeEach(() => {
        questions = [];
    });

    it('asks the user and sends the request when they accept', async () => {
        const target = await callJson(asking, 'run_request', { name: 'users/list', file: file(), env: 'production' });
        assert.deepEqual([target.status, received, questions.length], ['sent', ['GET /users'], 1]);
    });

    it('sends nothing when the user declines', async () => {
        const target = await callJson(asking, 'run_request', { name: 'users/post', file: file(), env: 'local' });
        assert.deepEqual([target.status, target.sent, received], ['declined', false, []]);
    });

    it('does not ask about an unmarked read', async () => {
        await callJson(asking, 'run_request', { name: 'users/list', file: file(), env: 'local' });
        assert.deepEqual(questions, []);
    });
});

describe('run_request cancelled while the user is being asked', () => {
    let cancellation;
    let late;

    before(async () => {
        late = await connect(workspace, () => {
            cancellation.abort();
            return new Promise(resolve => setTimeout(() => resolve({ action: 'accept' }), 100));
        });
    });

    after(async () => {
        await late.close();
    });

    it('sends nothing even if the user says yes afterwards', async () => {
        cancellation = new AbortController();
        const call = late.callTool({ name: 'run_request', arguments: { name: 'users/post', file: file(), env: 'local' } }, undefined, { signal: cancellation.signal });
        await assert.rejects(call);
        await new Promise(resolve => setTimeout(resolve, 300));
        assert.deepEqual(received, []);
    });
});
