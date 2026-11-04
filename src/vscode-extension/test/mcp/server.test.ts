import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { InMemoryTransport } from '@modelcontextprotocol/sdk/inMemory.js';
import { ElicitationCompleteNotificationSchema, ElicitRequestSchema } from '@modelcontextprotocol/sdk/types.js';
import { askForRedirectUrl, confirmWithUser, createServer, openAuthorizationUrl } from '../../src/mcp/server';
import { RESOURCE_BODIES, surface } from '../../src/mcp/surface';

async function connectedClient(): Promise<Client> {
    const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
    const client = new Client({ name: 'test', version: '0.0.0' });
    await Promise.all([createServer().connect(serverTransport), client.connect(clientTransport)]);
    return client;
}

describe('createServer', () => {
    it('advertises instructions with the resource URIs resolved', async () => {
        const target = await connectedClient();
        const instructions = target.getInstructions() ?? '';
        expect(instructions).toContain(surface.resources['language-definition'].uri);
        expect(instructions).toContain(surface.resources['idioms'].uri);
    });

    it('advertises instructions with no unresolved placeholders', async () => {
        const target = await connectedClient();
        expect(target.getInstructions() ?? '').not.toMatch(/\{(language_definition_uri|idioms_uri)\}/);
    });

    it('serves the markdown body of every declared resource', async () => {
        const target = await connectedClient();
        for (const definition of Object.values(surface.resources)) {
            const result = await target.readResource({ uri: definition.uri });
            expect(result.contents).toHaveLength(1);
            const [content] = result.contents as { uri: string; mimeType?: string; text?: string }[];
            expect(content.mimeType).toBe('text/markdown');
            expect((content.text ?? '').length).toBeGreaterThan(1000);
        }
    });

    it('serves every resource declared in the shared surface', async () => {
        const target = await connectedClient();
        const { resources } = await target.listResources();
        expect(resources.map(r => r.uri).sort()).toEqual(
            Object.values(surface.resources).map(d => d.uri).sort()
        );
    });

    it('exposes the documentation as a tool for clients that cannot read resources', async () => {
        const target = await connectedClient();
        const { tools } = await target.listTools();
        expect(tools.map(t => t.name).sort()).toEqual(
            ['get_rq_reference', 'lint_rq', 'list_requests', 'run_request', 'validate_rq']
        );
    });

    it('returns the grammar reference through the tool', async () => {
        const target = await connectedClient();
        const result = await target.callTool({
            name: 'get_rq_reference',
            arguments: { doc: 'language-definition' }
        });
        const [content] = result.content as { type: string; text: string }[];
        expect(result.isError).toBeFalsy();
        expect(content.text).toBe(RESOURCE_BODIES[surface.resources['language-definition'].uri]);
    });

    it('returns the idioms guide through the tool', async () => {
        const target = await connectedClient();
        const result = await target.callTool({ name: 'get_rq_reference', arguments: { doc: 'idioms' } });
        const [content] = result.content as { type: string; text: string }[];
        expect(content.text).toBe(RESOURCE_BODIES[surface.resources['idioms'].uri]);
    });

    it('rejects an unknown document instead of returning empty text', async () => {
        const target = await connectedClient();
        const result = await target.callTool({ name: 'get_rq_reference', arguments: { doc: 'nope' } });
        expect(result.isError).toBe(true);
    });
});

const SUMMARY = { request: 'users/delete', method: 'DELETE', url: 'http://x/users/1', environment: 'production' };

type Answer = { action: 'accept' | 'decline' | 'cancel'; content?: Record<string, string> };

async function serverWithClient(answer?: (message: string) => Answer | Promise<Answer>, schemas: unknown[] = []) {
    const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
    const server = createServer();
    const client = new Client({ name: 'test', version: '0.0.0' }, { capabilities: answer ? { elicitation: { form: {} } } : {} });
    const asked: string[] = [];
    if (answer) {
        client.setRequestHandler(ElicitRequestSchema, request => {
            asked.push(request.params.message);
            schemas.push('requestedSchema' in request.params ? request.params.requestedSchema : undefined);
            return answer(request.params.message);
        });
    }
    await Promise.all([server.connect(serverTransport), client.connect(clientTransport)]);
    return { server, asked };
}

describe('confirmWithUser', () => {
    it('reports confirmation as unavailable when the client cannot elicit', async () => {
        const { server } = await serverWithClient();
        expect(await confirmWithUser(server, SUMMARY)).toBe('unavailable');
    });

    it('accepts when the user accepts', async () => {
        const { server } = await serverWithClient(() => ({ action: 'accept' }));
        expect(await confirmWithUser(server, SUMMARY)).toBe('accepted');
    });

    it('declines when the user declines', async () => {
        const { server } = await serverWithClient(() => ({ action: 'decline' }));
        expect(await confirmWithUser(server, SUMMARY)).toBe('declined');
    });

    it('asks with no fields, so the client shows only accept and decline', async () => {
        const schemas: unknown[] = [];
        const { server } = await serverWithClient(() => ({ action: 'accept' }), schemas);
        await confirmWithUser(server, SUMMARY);
        expect(schemas).toEqual([{ type: 'object', properties: {} }]);
    });

    it('declines when the user dismisses the question', async () => {
        const { server } = await serverWithClient(() => ({ action: 'cancel' }));
        expect(await confirmWithUser(server, SUMMARY)).toBe('declined');
    });

    it('reports the question as unanswered once it times out', async () => {
        const { server } = await serverWithClient(() => new Promise<Answer>(() => undefined));
        expect(await confirmWithUser(server, SUMMARY, undefined, 20)).toBe('unanswered');
    });

    it('declines once the tool call is cancelled while the question is pending', async () => {
        const cancellation = new AbortController();
        const { server } = await serverWithClient(() => {
            cancellation.abort();
            return new Promise<Answer>(() => undefined);
        });
        expect(await confirmWithUser(server, SUMMARY, cancellation.signal)).toBe('declined');
    });

    it('declines a yes given after the tool call was cancelled', async () => {
        const cancellation = new AbortController();
        const { server } = await serverWithClient(() => {
            cancellation.abort();
            return { action: 'accept' };
        });
        expect(await confirmWithUser(server, SUMMARY, cancellation.signal)).toBe('declined');
    });

    it('shows the user the method, url and environment', async () => {
        const { server, asked } = await serverWithClient(() => ({ action: 'decline' }));
        await confirmWithUser(server, SUMMARY);
        expect(asked).toEqual(['Run DELETE http://x/users/1 against the `production` environment?']);
    });
});

async function serverWithUrlClient(elicitation: Record<string, unknown>, answer: () => Answer | Promise<Answer> = () => ({ action: 'accept' })) {
    const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
    const server = createServer();
    const client = new Client({ name: 'test', version: '0.0.0' }, { capabilities: { elicitation } });
    const asked: unknown[] = [];
    client.setRequestHandler(ElicitRequestSchema, request => {
        asked.push(request.params);
        return answer();
    });
    await Promise.all([server.connect(serverTransport), client.connect(clientTransport)]);
    return { server, client, asked };
}

const AUTHORIZATION_URL = 'https://auth.example.com/authorize?client_id=app';
const FINISHED = Promise.resolve();

describe('openAuthorizationUrl', () => {
    it('reports the url as unavailable when the client cannot open urls', async () => {
        const { server } = await serverWithUrlClient({ form: {} });
        expect(await openAuthorizationUrl(server, AUTHORIZATION_URL, 'login', FINISHED)).toBe('unavailable');
    });

    it('accepts when the user agrees to open the url', async () => {
        const { server } = await serverWithUrlClient({ url: {} });
        expect(await openAuthorizationUrl(server, AUTHORIZATION_URL, 'login', FINISHED)).toBe('accepted');
    });

    it('declines when the user refuses to open the url', async () => {
        const { server } = await serverWithUrlClient({ url: {} }, () => ({ action: 'decline' }));
        expect(await openAuthorizationUrl(server, AUTHORIZATION_URL, 'login', FINISHED)).toBe('declined');
    });

    it('reports the question as unanswered once it times out', async () => {
        const { server } = await serverWithUrlClient({ url: {} }, () => new Promise<Answer>(() => undefined));
        expect(await openAuthorizationUrl(server, AUTHORIZATION_URL, 'login', FINISHED, undefined, 20)).toBe('unanswered');
    });

    it('tells the client the sign-in finished, with the id it asked with', async () => {
        let finish: () => void = () => undefined;
        const finished = new Promise<void>(resolve => { finish = resolve; });
        const { server, client, asked } = await serverWithUrlClient({ url: {} }, () => {
            finish();
            return new Promise<Answer>(() => undefined);
        });
        const completed = new Promise<unknown>(resolve => {
            client.setNotificationHandler(ElicitationCompleteNotificationSchema, notification => resolve(notification.params.elicitationId));
        });
        openAuthorizationUrl(server, AUTHORIZATION_URL, 'login', finished, undefined, 1000).catch(() => undefined);
        expect(await completed).toBe((asked[0] as { elicitationId: string }).elicitationId);
    });

    it('asks the client to open the url and names the auth', async () => {
        const { server, asked } = await serverWithUrlClient({ url: {} });
        await openAuthorizationUrl(server, AUTHORIZATION_URL, 'login', FINISHED);
        expect(asked).toEqual([expect.objectContaining({
            mode: 'url',
            url: AUTHORIZATION_URL,
            message: 'Open this page and sign in to authorize `login`, so rq can send the request'
        })]);
    });
});

const PASTE_AUTH = {
    name: 'login',
    auth_type: 'oauth2_authorization_code',
    fields: { redirect_uri: 'https://app.example.com/callback' }
};

describe('askForRedirectUrl', () => {
    it('reports the dialog as unavailable when the client cannot show forms', async () => {
        const { server } = await serverWithClient();
        expect(await askForRedirectUrl(server, AUTHORIZATION_URL, PASTE_AUTH)).toEqual({ status: 'unavailable' });
    });

    it('returns the address the user pasted, without surrounding spaces', async () => {
        const { server } = await serverWithClient(() => ({ action: 'accept', content: { redirect_url: '  https://app.example.com/callback?code=x  ' } }));
        expect(await askForRedirectUrl(server, AUTHORIZATION_URL, PASTE_AUTH)).toEqual({ status: 'pasted', url: 'https://app.example.com/callback?code=x' });
    });

    it('declines when the user dismisses the dialog', async () => {
        const { server } = await serverWithClient(() => ({ action: 'cancel' }));
        expect(await askForRedirectUrl(server, AUTHORIZATION_URL, PASTE_AUTH)).toEqual({ status: 'declined' });
    });

    it('reports the dialog as unanswered once it times out', async () => {
        const { server } = await serverWithClient(() => new Promise<Answer>(() => undefined));
        expect(await askForRedirectUrl(server, AUTHORIZATION_URL, PASTE_AUTH, undefined, 20)).toEqual({ status: 'unanswered' });
    });

    it('asks for one required text field', async () => {
        const schemas: unknown[] = [];
        const { server } = await serverWithClient(() => ({ action: 'cancel' }), schemas);
        await askForRedirectUrl(server, AUTHORIZATION_URL, PASTE_AUTH);
        expect(schemas).toEqual([{ type: 'object', properties: { redirect_url: { type: 'string', title: 'Redirect URL' } }, required: ['redirect_url'] }]);
    });

    it('shows the sign-in url and the address the browser will end on', async () => {
        const { server, asked } = await serverWithClient(() => ({ action: 'cancel' }));
        await askForRedirectUrl(server, AUTHORIZATION_URL, PASTE_AUTH);
        expect([asked[0].includes(AUTHORIZATION_URL), asked[0].includes('it starts with https://app.example.com/callback')]).toEqual([true, true]);
    });
});

describe('run_request', () => {
    it('is annotated as a destructive, open-world tool', async () => {
        const target = await connectedClient();
        const { tools } = await target.listTools();
        expect(tools.find(t => t.name === 'run_request')?.annotations).toMatchObject({ destructiveHint: true, readOnlyHint: false, openWorldHint: true });
    });
});
