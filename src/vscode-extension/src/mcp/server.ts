import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { ErrorCode, McpError } from '@modelcontextprotocol/sdk/types.js';
import { z } from 'zod';
import { buildGenerateRqPrompt, render, RESOURCE_BODIES, surface } from './surface';
import { ConfirmationOutcome, getRqReference, listRequests, lintRq, ReferenceDoc, referenceDocIds, RequestSummary, runRequest, useDirectWasm, validateRq } from './tools';

const SERVER_NAME = 'rq-mcp';
const SERVER_VERSION = process.env.RQ_MCP_VERSION ?? '1.0.0';
export const CONFIRMATION_TIMEOUT_MS = 5 * 60 * 1000;

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

    server.registerTool('get_rq_reference', {
        description: surface.tools.get_rq_reference.description,
        inputSchema: { doc: z.enum(referenceDocIds() as [string, ...string[]]) }
    }, args => {
        try {
            return { content: [{ type: 'text' as const, text: getRqReference(args as { doc: ReferenceDoc }) }] };
        } catch (err) {
            return errorResult(err instanceof Error ? err.message : String(err));
        }
    });

    server.registerTool('run_request', {
        description: surface.tools.run_request.description,
        inputSchema: {
            name: z.string(),
            file: z.string().optional(),
            workspace_path: z.string().optional(),
            env: z.string().optional(),
            variables: z.record(z.string(), z.string()).optional(),
            confirmation_token: z.string().optional()
        },
        annotations: { readOnlyHint: false, destructiveHint: true, idempotentHint: false, openWorldHint: true }
    }, async (args, extra) => {
        try {
            return jsonResult(await runRequest(args, summary => confirmWithUser(server, summary, extra.signal), extra.signal));
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

export async function confirmWithUser(
    server: McpServer,
    summary: RequestSummary,
    signal?: AbortSignal,
    timeout = CONFIRMATION_TIMEOUT_MS
): Promise<ConfirmationOutcome> {
    const elicitation = server.server.getClientCapabilities()?.elicitation;
    if (!elicitation || (elicitation.url && !elicitation.form)) {
        return 'unavailable';
    }
    try {
        const result = await server.server.elicitInput({
            mode: 'form',
            message: confirmationMessage(summary),
            requestedSchema: { type: 'object', properties: {} }
        }, { signal, timeout });
        return result.action === 'accept' && !signal?.aborted ? 'accepted' : 'declined';
    } catch (err) {
        if (signal?.aborted) {
            return 'declined';
        }
        if (err instanceof McpError && err.code === ErrorCode.RequestTimeout) {
            return 'unanswered';
        }
        throw err;
    }
}

function confirmationMessage(summary: RequestSummary): string {
    const environmentClause = summary.environment
        ? surface.run_request.environment_clause.replace('{environment}', summary.environment)
        : '';
    return surface.run_request.confirmation_message
        .replace('{method}', summary.method)
        .replace('{url}', summary.url)
        .replace('{environment_clause}', environmentClause);
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
