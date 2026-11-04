import assert from 'node:assert/strict';
import http from 'node:http';
import net from 'node:net';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { after, afterEach, before, beforeEach, describe, it } from 'node:test';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';
import { ElicitationCompleteNotificationSchema, ElicitRequestSchema } from '@modelcontextprotocol/sdk/types.js';
import { callJson, connect, tempWorkspace } from './client.mjs';

const serverEntry = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', 'dist', 'server.js');

let api;
let received;
let tokens;
let workspace;
let redirectPort;

function freePort() {
    return new Promise((resolve, reject) => {
        const probe = net.createServer();
        probe.on('error', reject);
        probe.listen(0, '127.0.0.1', () => {
            const { port } = probe.address();
            probe.close(() => resolve(port));
        });
    });
}

function apiSource(apiPort) {
    return [
        `auth login(auth_type.oauth2_authorization_code) {`,
        '    client_id: "app",',
        `    authorization_url: "http://127.0.0.1:${apiPort}/authorize",`,
        `    token_url: "http://127.0.0.1:${apiPort}/token",`,
        `    redirect_uri: "http://127.0.0.1:${redirectPort}/callback",`,
        '}',
        '',
        `auth vscode_login(auth_type.oauth2_authorization_code) {`,
        '    client_id: "app",',
        `    authorization_url: "http://127.0.0.1:${apiPort}/authorize",`,
        `    token_url: "http://127.0.0.1:${apiPort}/token",`,
        '}',
        '',
        `auth paste_login(auth_type.oauth2_authorization_code) {`,
        '    client_id: "app",',
        `    authorization_url: "http://127.0.0.1:${apiPort}/authorize",`,
        `    token_url: "http://127.0.0.1:${apiPort}/token",`,
        `    redirect_uri: "${PASTED_REDIRECT_URI}",`,
        '}',
        '',
        `auth implicit_login(auth_type.oauth2_implicit) {`,
        '    client_id: "app",',
        `    authorization_url: "http://127.0.0.1:${apiPort}/authorize",`,
        `    redirect_uri: "${PASTED_REDIRECT_URI}",`,
        '}',
        '',
        '[auth("paste_login")]',
        `rq pasted("http://127.0.0.1:${apiPort}/pasted");`,
        '',
        '[auth("paste_login")]',
        '[method(POST)]',
        `rq pasted_write("http://127.0.0.1:${apiPort}/pasted", body: \${"name": "ada"});`,
        '',
        '[auth("implicit_login")]',
        `rq implicit("http://127.0.0.1:${apiPort}/implicit");`,
        '',
        '[auth("login")]',
        `rq me("http://127.0.0.1:${apiPort}/me");`,
        '',
        '[auth("login")]',
        '[method(POST)]',
        `rq rename("http://127.0.0.1:${apiPort}/me", body: \${"name": "ada"});`,
        '',
        '[auth("vscode_login")]',
        `rq other("http://127.0.0.1:${apiPort}/other");`,
        ''
    ].join('\n');
}

const PASTED_REDIRECT_URI = 'https://app.example.com/callback';

function followLink(link) {
    return new Promise((resolve, reject) => {
        http.get(link, res => {
            res.resume();
            res.on('end', () => resolve(res.headers.location ?? link));
        }).on('error', reject);
    });
}

async function landedOn(link) {
    const authorizationUrl = await followLink(link);
    return `${PASTED_REDIRECT_URI}?code=granted&state=${new URL(authorizationUrl).searchParams.get('state')}`;
}

async function signIn(link) {
    const url = new URL(link.includes('/authorize') ? link : await followLink(link));
    const redirect = `${url.searchParams.get('redirect_uri')}?code=granted&state=${url.searchParams.get('state')}`;
    return new Promise((resolve, reject) => {
        http.get(redirect, res => {
            res.resume();
            res.on('end', resolve);
        }).on('error', reject);
    });
}

async function connectOpeningUrls(answer, onComplete = () => undefined) {
    const client = new Client({ name: 'rq-mcp-test', version: '0.0.0' }, { capabilities: { elicitation: { url: {}, form: {} } } });
    client.setRequestHandler(ElicitRequestSchema, request => answer(request.params));
    client.setNotificationHandler(ElicitationCompleteNotificationSchema, notification => onComplete(notification.params.elicitationId));
    await client.connect(new StdioClientTransport({ command: process.execPath, args: [serverEntry], cwd: workspace, stderr: 'ignore' }));
    return client;
}

before(async () => {
    api = http.createServer((req, res) => {
        if (req.url === '/token') {
            tokens += 1;
            res.setHeader('content-type', 'application/json');
            res.end(JSON.stringify({ access_token: `issued-token-${tokens}`, token_type: 'Bearer', expires_in: 3600 }));
            return;
        }
        received.push(`${req.method} ${req.url} ${req.headers.authorization ?? ''}`);
        res.setHeader('content-type', 'application/json');
        res.end(JSON.stringify({ path: req.url }));
    });
    await new Promise(resolve => api.listen(0, '127.0.0.1', resolve));
    redirectPort = await freePort();
    workspace = tempWorkspace({ 'api.rq': apiSource(api.address().port) });
});

after(async () => {
    await new Promise(resolve => api.close(resolve));
});

beforeEach(() => {
    received = [];
    tokens = 0;
});

const file = () => path.join(workspace, 'api.rq');

describe('run_request with a sign-in, on a client that cannot open urls', () => {
    let client;

    beforeEach(async () => {
        client = await connect(workspace);
    });

    afterEach(async () => {
        await client.close();
    });

    it('returns the authorization url and sends nothing', async () => {
        const target = await callJson(client, 'run_request', { name: 'me', file: file() });
        const authorizationUrl = await followLink(target.authorization_url);
        await signIn(authorizationUrl);
        assert.deepEqual([target.status, target.sent, new URL(authorizationUrl).pathname, received], ['authorization_required', false, '/authorize', []]);
    });

    it('sends the request with the token once the user signed in', async () => {
        const { authorization_url } = await callJson(client, 'run_request', { name: 'me', file: file() });
        await signIn(authorization_url);
        const target = await callJson(client, 'run_request', { name: 'me', file: file() });
        assert.deepEqual([target.status, received], ['sent', ['GET /me Bearer issued-token-1']]);
    });

    it('redacts the token in the headers it reports', async () => {
        const { authorization_url } = await callJson(client, 'run_request', { name: 'me', file: file() });
        await signIn(authorization_url);
        const target = await callJson(client, 'run_request', { name: 'me', file: file() });
        assert.equal(JSON.stringify(target).includes('issued-token'), false);
    });

    it('keeps the token for the next request without another sign-in', async () => {
        const { authorization_url } = await callJson(client, 'run_request', { name: 'me', file: file() });
        await signIn(authorization_url);
        await callJson(client, 'run_request', { name: 'me', file: file() });
        const target = await callJson(client, 'run_request', { name: 'me', file: file() });
        assert.deepEqual([target.status, tokens], ['sent', 1]);
    });

    it('signs in before asking to confirm a write', async () => {
        const args = { name: 'rename', file: file() };
        const { authorization_url } = await callJson(client, 'run_request', args);
        await signIn(authorization_url);
        const target = await callJson(client, 'run_request', args);
        assert.deepEqual([target.status, received], ['confirmation_required', []]);
    });

    it('refuses an auth whose redirect only VS Code can receive', async () => {
        const target = await callJson(client, 'run_request', { name: 'other', file: file() });
        assert.deepEqual([target.status, target.sent, target.message.includes('the VS Code default'), received], ['authorization_failed', false, true, []]);
    });
});

describe('run_request with a pasted redirect address, on a client without dialogs', () => {
    let client;

    before(async () => {
        client = await connect(workspace);
    });

    after(async () => {
        await client.close();
    });

    it('asks for the address the browser ends on and sends nothing', async () => {
        const target = await callJson(client, 'run_request', { name: 'pasted', file: file() });
        assert.deepEqual([target.status, target.redirect_url_required, target.message.includes('redirect_url'), received], ['authorization_required', true, true, []]);
    });

    it('sends the request with the token once called with the address the user pasted', async () => {
        const args = { name: 'pasted', file: file() };
        const { authorization_url } = await callJson(client, 'run_request', args);
        const target = await callJson(client, 'run_request', { ...args, redirect_url: await landedOn(authorization_url) });
        assert.deepEqual([target.status, received], ['sent', ['GET /pasted Bearer issued-token-1']]);
    });

    it('sends a confirmed write when the retry repeats the address the user pasted', async () => {
        const fresh = await connect(workspace);
        const args = { name: 'pasted_write', file: file() };
        const { authorization_url } = await callJson(fresh, 'run_request', args);
        const redirect_url = await landedOn(authorization_url);
        const held = await callJson(fresh, 'run_request', { ...args, redirect_url });
        const target = await callJson(fresh, 'run_request', { ...args, redirect_url, confirmation_token: held.confirmation_token });
        await fresh.close();
        assert.deepEqual([held.status, target.status, received], ['confirmation_required', 'sent', ['POST /pasted Bearer issued-token-1']]);
    });

    it('refuses the implicit flow, whose token would reach the agent', async () => {
        const target = await callJson(client, 'run_request', { name: 'implicit', file: file() });
        assert.deepEqual([target.status, target.message.includes('implicit flow'), received], ['authorization_failed', true, []]);
    });

    it('keeps waiting after an address that is not the redirect uri, and explains why', async () => {
        const fresh = await connect(workspace);
        const args = { name: 'pasted', file: file() };
        const { authorization_url } = await callJson(fresh, 'run_request', args);
        const wrong = await callJson(fresh, 'run_request', { ...args, redirect_url: 'https://evil.example.com/callback?code=stolen' });
        const target = await callJson(fresh, 'run_request', { ...args, redirect_url: await landedOn(authorization_url) });
        await fresh.close();
        assert.deepEqual([wrong.status, wrong.message.includes('still waiting for the right address'), target.status], ['authorization_failed', true, 'sent']);
    });
});

describe('run_request with a pasted redirect address, on a client with dialogs', () => {
    let client;

    before(async () => {
        client = await connect(workspace, async message => {
            const authorizationUrl = await followLink(message.match(/http:\/\/127\.0\.0\.1:\d+\/[0-9a-f]{32}/)[0]);
            const state = new URL(authorizationUrl).searchParams.get('state');
            const redirect_url = message.includes('implicit_login')
                ? `${PASTED_REDIRECT_URI}#access_token=implicit-token&state=${state}`
                : `${PASTED_REDIRECT_URI}?code=granted&state=${state}`;
            return { action: 'accept', content: { redirect_url } };
        });
    });

    after(async () => {
        await client.close();
    });

    it('takes the address from the dialog and sends the request in the same call', async () => {
        const target = await callJson(client, 'run_request', { name: 'pasted', file: file() });
        assert.deepEqual([target.status, received], ['sent', ['GET /pasted Bearer issued-token-1']]);
    });

    it('takes the token of the implicit flow from the dialog', async () => {
        const target = await callJson(client, 'run_request', { name: 'implicit', file: file() });
        assert.deepEqual([target.status, received], ['sent', ['GET /implicit Bearer implicit-token']]);
    });
});

describe('run_request with an address pasted without its state', () => {
    let client;

    before(async () => {
        client = await connect(workspace, () => ({ action: 'accept', content: { redirect_url: `${PASTED_REDIRECT_URI}?code=granted` } }));
    });

    after(async () => {
        await client.close();
    });

    it('sends nothing and explains that the sign-in link was most likely cut off', async () => {
        const target = await callJson(client, 'run_request', { name: 'pasted', file: file() });
        assert.deepEqual(
            [target.status, target.sent, target.message.startsWith('Nothing was sent: signing in to `paste_login` failed.'), target.message.includes('cut off before its state'), tokens, received],
            ['authorization_failed', false, true, true, 0, []]
        );
    });
});

describe('run_request with a sign-in, on a client that opens urls', () => {
    let client;
    let asked;

    before(async () => {
        client = await connectOpeningUrls(async params => {
            asked.push(params.mode);
            if (params.mode === 'url') {
                await signIn(params.url);
            }
            return { action: 'accept' };
        });
    });

    beforeEach(() => {
        asked = [];
    });

    after(async () => {
        await client.close();
    });

    it('asks the client to open the url and sends the request in the same call', async () => {
        const target = await callJson(client, 'run_request', { name: 'me', file: file() });
        assert.deepEqual([target.status, asked, received], ['sent', ['url'], ['GET /me Bearer issued-token-1']]);
    });
});

describe('run_request with a sign-in, on a client that keeps the page open until told the sign-in finished', () => {
    let client;
    const completions = new Map();
    const opened = [];
    const closed = [];

    before(async () => {
        client = await connectOpeningUrls(async params => {
            opened.push(params.elicitationId);
            const completed = new Promise(resolve => completions.set(params.elicitationId, resolve));
            await signIn(params.url);
            await completed;
            return { action: 'accept' };
        }, id => {
            closed.push(id);
            completions.get(id)?.();
        });
    });

    after(async () => {
        await client.close();
    });

    it('sends the request', async () => {
        const target = await callJson(client, 'run_request', { name: 'me', file: file() });
        assert.deepEqual([target.status, received], ['sent', ['GET /me Bearer issued-token-1']]);
    });

    it('tells the client the sign-in finished, so it closes the page', async () => {
        assert.deepEqual([opened.length, closed], [1, opened]);
    });
});

describe('run_request with a sign-in the user declines', () => {
    let client;

    before(async () => {
        client = await connectOpeningUrls(async () => ({ action: 'decline' }));
    });

    after(async () => {
        await client.close();
    });

    it('sends nothing', async () => {
        const target = await callJson(client, 'run_request', { name: 'me', file: file() });
        assert.deepEqual([target.status, target.sent, received], ['declined', false, []]);
    });
});
