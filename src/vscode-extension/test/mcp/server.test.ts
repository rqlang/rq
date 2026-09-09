import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { InMemoryTransport } from '@modelcontextprotocol/sdk/inMemory.js';
import { createServer } from '../../src/mcp/server';
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
            ['get_rq_reference', 'lint_rq', 'list_requests', 'validate_rq']
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
