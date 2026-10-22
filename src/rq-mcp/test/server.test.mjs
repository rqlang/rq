import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { after, before, describe, it } from 'node:test';
import { connect } from './client.mjs';

const { version } = createRequire(import.meta.url)('../package.json');
const languageDefinitionUri = 'rqlang://docs/language-definition';
const idiomsUri = 'rqlang://docs/idioms';

let client;

before(async () => {
    client = await connect();
});

after(async () => {
    await client.close();
});

describe('server', () => {
    it('identifies itself with the package version', () => {
        assert.deepEqual(client.getServerVersion(), { name: 'rq-mcp', version });
    });

    it('points clients without resource support at the reference tool', () => {
        assert.match(client.getInstructions(), /get_rq_reference/);
    });

    it('serves the authoring tools', async () => {
        const target = (await client.listTools()).tools.map(t => t.name).sort();
        assert.deepEqual(target, ['get_rq_reference', 'lint_rq', 'list_requests', 'validate_rq']);
    });
});

describe('resources', () => {
    it('lists both rqlang documents', async () => {
        const target = (await client.listResources()).resources.map(r => r.uri).sort();
        assert.deepEqual(target, [idiomsUri, languageDefinitionUri]);
    });

    it('serves the language definition as markdown', async () => {
        const [target] = (await client.readResource({ uri: languageDefinitionUri })).contents;
        assert.equal(target.mimeType, 'text/markdown');
        assert.ok(target.text.length > 1000);
    });

    it('serves the same body through the reference tool', async () => {
        const [resource] = (await client.readResource({ uri: idiomsUri })).contents;
        const target = await client.callTool({ name: 'get_rq_reference', arguments: { doc: 'idioms' } });
        assert.equal(target.content[0].text, resource.text);
    });

    it('rejects an unknown uri', async () => {
        await assert.rejects(client.readResource({ uri: 'rqlang://docs/unknown' }));
    });
});

describe('generate_rq prompt', () => {
    it('embeds the intent', async () => {
        const target = await client.getPrompt({ name: 'generate_rq', arguments: { intent: 'create-user POST' } });
        assert.match(target.messages[0].content.text, /create-user POST/);
    });
});
