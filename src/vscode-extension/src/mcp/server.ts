import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { z } from 'zod';
import { buildGenerateRqPrompt, render, RESOURCE_BODIES, surface } from './surface';
import { listRequests, lintRq, useDirectWasm, validateRq } from './tools';

const SERVER_NAME = 'rq-mcp';
const SERVER_VERSION = process.env.RQ_MCP_VERSION ?? '1.0.0';

function jsonResult(payload: unknown) {
    return { content: [{ type: 'text' as const, text: JSON.stringify(payload, null, 2) }] };
}

function errorResult(message: string) {
    return { content: [{ type: 'text' as const, text: message }], isError: true };
}

export function createServer(): McpServer {
    const server = new McpServer(
        { name: SERVER_NAME, version: SERVER_VERSION },
        { instructions: render(surface.instructions), capabilities: { tools: {}, resources: {}, prompts: {} } }
    );

    server.registerTool('validate_rq', {
        description: surface.tools.validate_rq.description,
        inputSchema: {
            source: z.string(),
            path: z.string().optional(),
            workspace_path: z.string().optional(),
            env: z.string().optional()
        }
    }, async args => {
        try {
            return jsonResult(await validateRq(args));
        } catch (err) {
            return errorResult(err instanceof Error ? err.message : String(err));
        }
    });

    server.registerTool('lint_rq', {
        description: surface.tools.lint_rq.description,
        inputSchema: {
            source: z.string(),
            path: z.string().optional(),
            workspace_path: z.string().optional()
        }
    }, async args => {
        try {
            return jsonResult(await lintRq(args));
        } catch (err) {
            return errorResult(err instanceof Error ? err.message : String(err));
        }
    });

    server.registerTool('list_requests', {
        description: surface.tools.list_requests.description,
        inputSchema: { path: z.string().optional() }
    }, async args => {
        try {
            return jsonResult(await listRequests(args));
        } catch (err) {
            return errorResult(err instanceof Error ? err.message : String(err));
        }
    });

    for (const definition of Object.values(surface.resources)) {
        server.registerResource(definition.title, definition.uri, {
            title: definition.title,
            description: definition.description,
            mimeType: 'text/markdown'
        }, async uri => ({
            contents: [{ uri: uri.href, mimeType: 'text/markdown', text: RESOURCE_BODIES[definition.uri] }]
        }));
    }

    server.registerPrompt('generate_rq', {
        description: surface.prompt.description,
        argsSchema: { intent: z.string(), workspace_path: z.string().optional() }
    }, ({ intent, workspace_path }) => ({
        description: surface.prompt.description,
        messages: [{
            role: 'user' as const,
            content: { type: 'text' as const, text: buildGenerateRqPrompt(intent, workspace_path) }
        }]
    }));

    return server;
}

async function main(): Promise<void> {
    useDirectWasm();
    await createServer().connect(new StdioServerTransport());
}

if (require.main === module) {
    main().catch(err => {
        console.error('rq-mcp failed to start:', err);
        process.exit(1);
    });
}
