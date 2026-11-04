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
        '',
        'rq endless("{{base_url}}/endless");',
        '',
        'rq stalled("{{base_url}}/stalled");',
        '',
        'let top = "1";',
        '',
        'rq paged("{{base_url}}/paged?top={{top}}");',
        '',
        'let service_token = "service-token";',
        '',
        'auth service_auth(auth_type.bearer) {',
        '    token: "{{service_token}}",',
        '}',
        '',
        '[auth("service_auth")]',
        'rq service("{{base_url}}/service");',
        '',
        'let token_url = "{{base_url}}/token";',
        '',
        'auth machine(auth_type.oauth2_client_credentials) {',
        '    client_id: "app",',
        '    client_secret: "machine-secret",',
        '    token_url: "{{token_url}}",',
        '}',
        '',
        '[auth("machine")]',
        'rq machine("{{base_url}}/machine");',
        ''
    ].join('\n');
}

before(async () => {
    api = http.createServer((req, res) => {
        received.push(`${req.method} ${req.url}`);
        if (req.url === '/stalled') {
            res.write('x'.repeat(60_000));
            return;
        }
        if (req.url === '/endless') {
            res.write('x'.repeat(300_000));
            return;
        }
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

    it('sends the request the user approved even if a variable it uses changed before the retry', async () => {
        const port = api.address().port;
        const source = target => `let target = "${target}";\n\n[method(POST)]\nrq moved("http://127.0.0.1:${port}/{{target}}", body: \${});\n`;
        const moved = tempWorkspace({ 'moved.rq': source('approved') });
        const own = await connect(moved);
        const args = { name: 'moved', file: path.join(moved, 'moved.rq') };
        const { confirmation_token } = await callJson(own, 'run_request', args);
        fs.writeFileSync(args.file, source('elsewhere'));
        const target = await callJson(own, 'run_request', { ...args, confirmation_token });
        await own.close();
        assert.deepEqual([target.status, received], ['sent', ['POST /approved']]);
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

    it('stops reading a response that never ends once the body is long enough', async () => {
        const target = await callJson(client, 'run_request', { name: 'endless', file: file(), env: 'local' });
        assert.deepEqual([target.response.body.length, target.response.body_truncated], [50_000, true]);
    });

    it('returns the body once it holds enough characters, even if the server keeps the response open', async () => {
        const target = await callJson(client, 'run_request', { name: 'stalled', file: file(), env: 'local' });
        assert.deepEqual([target.response.body.length, target.response.body_truncated], [50_000, true]);
    });

    it('sends a request with an overridden variable without asking', async () => {
        const target = await callJson(client, 'run_request', { name: 'paged', file: file(), env: 'local', variables: { top: '10' } });
        assert.deepEqual([target.status, received], ['sent', ['GET /paged?top=10']]);
    });

    it('lists the overridden variables in the request it sent', async () => {
        const target = await callJson(client, 'run_request', { name: 'paged', file: file(), env: 'local', variables: { top: '10' } });
        assert.deepEqual(target.request.variables, { top: '10' });
    });

    it('rejects a variable the request does not use', async () => {
        const target = await client.callTool({ name: 'run_request', arguments: { name: 'paged', file: file(), env: 'local', variables: { tpo: '10' } } });
        assert.deepEqual([target.isError, target.content[0].text.includes('`tpo`'), received], [true, true, []]);
    });

    it('holds an override that sends the request to another host', async () => {
        const target = await callJson(client, 'run_request', { name: 'users/list', file: file(), env: 'local', variables: { base_url: 'http://127.0.0.1:1' } });
        assert.deepEqual([target.status, typeof target.confirmation_token, received], ['confirmation_required', 'string', []]);
    });

    it('holds an override that changes the auth', async () => {
        const target = await callJson(client, 'run_request', { name: 'service', file: file(), env: 'local', variables: { service_token: 'other' } });
        assert.deepEqual([target.status, received], ['confirmation_required', []]);
    });

    it('asks before fetching a token with an overridden auth', async () => {
        const target = await callJson(client, 'run_request', { name: 'machine', file: file(), env: 'local', variables: { token_url: `http://127.0.0.1:${api.address().port}/stolen` } });
        assert.deepEqual([target.status, received], ['confirmation_required', []]);
    });

    it('sends a request that moved to another host once the user confirms the override', async () => {
        const other = http.createServer((req, res) => {
            received.push(`other ${req.method} ${req.url}`);
            res.end('{}');
        });
        await new Promise(resolve => other.listen(0, '127.0.0.1', resolve));
        const args = { name: 'users/list', file: file(), env: 'local', variables: { base_url: `http://127.0.0.1:${other.address().port}` } };
        const { confirmation_token } = await callJson(client, 'run_request', args);
        const target = await callJson(client, 'run_request', { ...args, confirmation_token });
        await new Promise(resolve => other.close(resolve));
        assert.deepEqual([target.status, received], ['sent', ['other GET /users']]);
    });

    it('refuses the confirmation token of an override once its request was sent', async () => {
        const args = { name: 'service', file: file(), env: 'local', variables: { service_token: 'other' } };
        const { confirmation_token } = await callJson(client, 'run_request', args);
        await callJson(client, 'run_request', { ...args, confirmation_token });
        const target = await client.callTool({ name: 'run_request', arguments: { ...args, confirmation_token } });
        assert.deepEqual([target.isError, received], [true, ['GET /service']]);
    });

    it('accepts the auth_token variable', async () => {
        const target = await callJson(client, 'run_request', { name: 'users/list', file: file(), env: 'local', variables: { auth_token: 'given' } });
        assert.equal(target.status, 'sent');
    });

    it('rejects a file outside the workspace', async () => {
        const outside = tempWorkspace({ 'leak.rq': `rq leak("http://127.0.0.1:${api.address().port}/leak?{{api_key}}");\n` });
        const target = await client.callTool({ name: 'run_request', arguments: { name: 'leak', file: path.join(outside, 'leak.rq') } });
        assert.deepEqual([target.isError, received], [true, []]);
    });

    it('rejects a link in the workspace to a file outside it', async () => {
        const outside = tempWorkspace({ 'leak.rq': `rq leak("http://127.0.0.1:${api.address().port}/leak?{{api_key}}");\n` });
        const link = path.join(workspace, 'linked.rq');
        fs.symlinkSync(path.join(outside, 'leak.rq'), link);
        const target = await client.callTool({ name: 'run_request', arguments: { name: 'leak', file: link } });
        fs.rmSync(link);
        assert.deepEqual([target.isError, received], [true, []]);
    });

    it('rejects a request it found through a link in the workspace to a file outside it', async () => {
        const outside = tempWorkspace({ 'found.rq': `rq found("http://127.0.0.1:${api.address().port}/found?{{api_key}}");\n` });
        const link = path.join(workspace, 'found.rq');
        fs.symlinkSync(path.join(outside, 'found.rq'), link);
        const target = await client.callTool({ name: 'run_request', arguments: { name: 'found', workspace_path: workspace } });
        fs.rmSync(link);
        assert.deepEqual([target.isError, target.content[0].text.includes('is not inside the workspace'), received], [true, true, []]);
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

    it('shows the user the whole url while redacting it for the agent', async () => {
        const target = await callJson(asking, 'run_request', { name: 'keyed', file: file(), env: 'production' });
        assert.deepEqual([questions[0].includes('?key=k3y-s3cret'), target.request.url.endsWith('?key=[redacted]')], [true, true]);
    });

    it('names the overridden variables when it asks about an override', async () => {
        await callJson(asking, 'run_request', { name: 'service', file: file(), env: 'local', variables: { service_token: 'other' } });
        assert.deepEqual([questions.length, questions[0]?.endsWith(' with service_token=other?')], [1, true]);
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
