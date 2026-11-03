import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { ErrorCode, McpError } from '@modelcontextprotocol/sdk/types.js';
import * as crypto from 'crypto';
import { z } from 'zod';
import { OAuthAuthConfig } from '../auth/types';
import { AuthorizationUrlOutcome, Authorizer, RedirectUrlOutcome } from './authorization';
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

    const authorizer = new Authorizer({
        openUrl: (url, auth, finished, signal) => openAuthorizationUrl(server, url, auth.name, finished, signal),
        askForRedirectUrl: (url, auth, signal) => askForRedirectUrl(server, url, auth, signal)
    }, message => process.stderr.write(`[OAuth] ${message}\n`));

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
            confirmation_token: z.string().optional(),
            redirect_url: z.string().optional()
        },
        annotations: { readOnlyHint: false, destructiveHint: true, idempotentHint: false, openWorldHint: true }
    }, async (args, extra) => {
        try {
            return jsonResult(await runRequest(args, {
                confirm: summary => confirmWithUser(server, summary, extra.signal),
                authorize: (auth, signal, redirectUrl) => authorizer.authorize(auth, signal, redirectUrl)
            }, extra.signal));
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
    if (!canAskWithForm(server)) {
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

export async function openAuthorizationUrl(
    server: McpServer,
    url: string,
    authName: string,
    finished: Promise<void>,
    signal?: AbortSignal,
    timeout = CONFIRMATION_TIMEOUT_MS
): Promise<AuthorizationUrlOutcome> {
    if (!server.server.getClientCapabilities()?.elicitation?.url) {
        return 'unavailable';
    }
    const elicitationId = crypto.randomUUID();
    const notifyCompletion = server.server.createElicitationCompletionNotifier(elicitationId);
    finished.then(() => notifyCompletion()).catch(() => undefined);
    try {
        const result = await server.server.elicitInput({
            mode: 'url',
            message: surface.run_request.authorization_message.replace('{auth}', authName),
            url,
            elicitationId
        }, { signal, timeout });
        return result.action === 'accept' && !signal?.aborted ? 'accepted' : 'declined';
    } catch (err) {
        if (signal?.aborted || (err instanceof McpError && err.code === ErrorCode.RequestTimeout)) {
            return 'declined';
        }
        throw err;
    }
}

export async function askForRedirectUrl(
    server: McpServer,
    authorizationUrl: string,
    auth: OAuthAuthConfig,
    signal?: AbortSignal,
    timeout = CONFIRMATION_TIMEOUT_MS
): Promise<RedirectUrlOutcome> {
    if (!canAskWithForm(server)) {
        return { status: 'unavailable' };
    }
    try {
        const result = await server.server.elicitInput({
            mode: 'form',
            message: surface.run_request.redirect_url_message
                .replace('{auth}', auth.name)
                .replace('{url}', authorizationUrl)
                .replace('{redirect_uri}', auth.fields.redirect_uri ?? ''),
            requestedSchema: {
                type: 'object',
                properties: { redirect_url: { type: 'string', title: 'Redirect URL' } },
                required: ['redirect_url']
            }
        }, { signal, timeout });
        const url = result.content?.redirect_url;
        if (result.action !== 'accept' || typeof url !== 'string' || signal?.aborted) {
            return { status: 'declined' };
        }
        return { status: 'pasted', url: url.trim() };
    } catch (err) {
        if (signal?.aborted || (err instanceof McpError && err.code === ErrorCode.RequestTimeout)) {
            return { status: 'declined' };
        }
        throw err;
    }
}

function canAskWithForm(server: McpServer): boolean {
    const elicitation = server.server.getClientCapabilities()?.elicitation;
    return Boolean(elicitation) && !(elicitation?.url && !elicitation.form);
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
