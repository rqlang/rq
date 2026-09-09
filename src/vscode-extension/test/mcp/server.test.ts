import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { InMemoryTransport } from '@modelcontextprotocol/sdk/inMemory.js';
import { createServer } from '../../src/mcp/server';
import { surface } from '../../src/mcp/surface';

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

    it('serves every resource declared in the shared surface', async () => {
        const target = await connectedClient();
        const { resources } = await target.listResources();
        expect(resources.map(r => r.uri).sort()).toEqual(
            Object.values(surface.resources).map(d => d.uri).sort()
        );
    });
});
