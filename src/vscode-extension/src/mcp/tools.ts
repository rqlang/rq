import * as crypto from 'crypto';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import { setWasmTransport, wasmCall } from '../wasmHost';
import { buildSecretsMap, draftsJson, isDirectory, normalizePath } from '../utils';
import { CancellationSignal, ExecuteRequestOptions, PreparedRequest, prepareRequest, requestDefinition, requestScope, sendRequest } from '../execution';
import { RESOURCE_BODIES, surface } from './surface';

const DRAFT_FILE_NAME = 'draft.rq';

export interface Diagnostic {
    severity: string;
    message: string;
    line: number;
    column: number;
    file?: string;
    rule?: string;
    suggested_fix?: string;
}

export interface CheckResult {
    ok: boolean;
    diagnostics: Diagnostic[];
}

export function useDirectWasm(): void {
    setWasmTransport('direct');
}

function posix(target: string): string {
    return normalizePath(target).replace(/\\/g, '/');
}

export function draftPath(workspacePath: string, filePath: string | undefined): string {
    const name = filePath ?? DRAFT_FILE_NAME;
    if (!path.isAbsolute(name)) { return posix(path.join(workspacePath, name)); }
    return posix(isInside(workspacePath, name) ? name : path.join(workspacePath, path.basename(name)));
}

export async function workspaceFor(workspacePath?: string, filePath?: string): Promise<string> {
    if (workspacePath) { return posix(await directoryContaining(workspacePath)); }
    if (filePath && path.isAbsolute(filePath)) {
        return posix(await existing((await isDirectory(filePath)) ? filePath : path.dirname(filePath)));
    }
    return posix(process.cwd());
}

export function isUnscoped(directory: string): boolean {
    const target = posix(path.resolve(directory)).replace(/\/$/, '');
    const root = posix(path.parse(path.resolve(directory)).root).replace(/\/$/, '');
    return target === root || target === posix(os.homedir()).replace(/\/$/, '');
}

function isInside(directory: string, filePath: string): boolean {
    const relative = path.relative(directory, filePath);
    return relative !== '' && !relative.startsWith('..') && !path.isAbsolute(relative);
}

async function statOf(target: string): Promise<fs.Stats> {
    try {
        return await fs.promises.stat(target);
    } catch (err) {
        throw accessError(target, err);
    }
}

async function existing(target: string): Promise<string> {
    await statOf(target);
    return target;
}

async function directoryContaining(target: string): Promise<string> {
    return (await statOf(target)).isDirectory() ? target : path.dirname(target);
}

function accessError(target: string, err: unknown): Error {
    const code = (err as NodeJS.ErrnoException).code;
    if (code === 'ENOENT' || code === 'ENOTDIR') { return new Error(`Path not found: ${target}`); }
    if (code === 'EACCES' || code === 'EPERM') { return new Error(`Permission denied: ${target}`); }
    return new Error(`Cannot access ${target}: ${err instanceof Error ? err.message : String(err)}`);
}

export async function validateRq(args: {
    source: string;
    path?: string;
    workspace_path?: string;
    env?: string;
}): Promise<CheckResult> {
    const workspace = await workspaceFor(args.workspace_path, args.path);
    const draft = draftPath(workspace, args.path);
    const filesJson = draftsJson([{ path: draft, source: args.source }]);
    const secretsJson = await buildSecretsMap(workspace);
    const raw = await wasmCall('check', [filesJson, secretsJson, draft, args.env]);
    const parsed = JSON.parse(raw) as { errors: Diagnostic[] };
    const diagnostics = (parsed.errors ?? []).map(e => ({ ...e, severity: e.severity ?? 'error' }));
    return { ok: diagnostics.length === 0, diagnostics };
}

export async function lintRq(args: { source: string; path?: string; workspace_path?: string }): Promise<CheckResult> {
    const workspace = await workspaceFor(args.workspace_path, args.path);
    const draft = draftPath(workspace, args.path);
    const filesJson = draftsJson([{ path: draft, source: args.source }]);
    const lintWorkspace = isUnscoped(workspace) ? undefined : workspace;
    const byPath = JSON.parse(await wasmCall('lint', [filesJson, lintWorkspace])) as Record<string, CheckResult>;
    return byPath[draft] ?? { ok: true, diagnostics: [] };
}

export async function listRequests(args: { path?: string }): Promise<unknown> {
    const target = args.path ? posix(await existing(args.path)) : await workspaceFor();
    const scanned = posix(await directoryContaining(target));
    if (isUnscoped(scanned)) {
        throw new Error(`Refusing to scan every file under ${scanned}: pass the project folder as \`path\`.`);
    }
    const filesJson = draftsJson();
    const secretsJson = await buildSecretsMap(target);
    const raw = await wasmCall('list_requests', [filesJson, secretsJson, target]);
    const parsed = JSON.parse(raw) as { requests: unknown[]; parse_errors?: Diagnostic[] };
    return { requests: parsed.requests ?? [], parse_errors: parsed.parse_errors ?? [] };
}

export type ReferenceDoc = keyof typeof surface.resources;

export function referenceDocIds(): ReferenceDoc[] {
    return Object.keys(surface.resources) as ReferenceDoc[];
}

export function getRqReference(args: { doc: ReferenceDoc }): string {
    const definition = surface.resources[args.doc];
    if (!definition) {
        throw new Error(`Unknown rqlang document \`${args.doc}\`. Available: ${referenceDocIds().join(', ')}.`);
    }
    return RESOURCE_BODIES[definition.uri];
}

export interface RequestSummary {
    request: string;
    method: string;
    url: string;
    environment: string | null;
}

export type ConfirmationOutcome = 'accepted' | 'declined' | 'unanswered' | 'unavailable';

export type ConfirmRequest = (summary: RequestSummary) => Promise<ConfirmationOutcome>;

export interface RunRequestArgs {
    name: string;
    file?: string;
    workspace_path?: string;
    env?: string;
    variables?: Record<string, string>;
    confirmation_token?: string;
}

export const MAX_RESPONSE_BODY_LENGTH = 50_000;
const AUTH_TOKEN_VARIABLE = 'auth_token';
const CONFIRMATION_TTL_MS = 5 * 60 * 1000;
const REDACTED = '[redacted]';
const SENSITIVE_HEADERS = new Set(['authorization', 'proxy-authorization', 'cookie', 'set-cookie', 'x-api-key', 'x-auth-token']);
const pendingConfirmations = new Map<string, { fingerprint: string; expiresAt: number }>();

export async function runRequest(args: RunRequestArgs, confirm: ConfirmRequest, signal?: AbortSignal): Promise<unknown> {
    const workspace = await workspaceFor(args.workspace_path);
    if (!args.file && isUnscoped(workspace)) {
        throw new Error(`Refusing to search every file under ${workspace}: pass the project folder as \`workspace_path\` or the request's \`file\`.`);
    }
    const scope = await requestScope(workspace);
    const options: ExecuteRequestOptions = {
        requestName: args.name.replace(/\./g, '/'),
        sourceFile: args.file ? await fileInside(workspace, args.file) : undefined,
        environment: args.env,
        variables: args.variables,
        cancellation: cancellationOf(signal)
    };
    const definition = await requestDefinition(scope, options) as { RequiredVariables?: string[] };
    rejectUndeclaredVariables(args.variables, definition.RequiredVariables ?? []);
    const prepared = await prepareRequest(scope, options);
    const summary = summaryOf(prepared, args.env);
    if (prepared.agent_confirmation_required) {
        const fingerprint = () => fingerprintOf(args, workspace, definition);
        const outcome = args.confirmation_token
            ? redeemConfirmation(args.confirmation_token, fingerprint())
            : await confirm(summary);
        if (outcome === 'declined') {
            return { status: 'declined', sent: false, request: summary };
        }
        if (outcome === 'unanswered') {
            return { status: 'confirmation_timeout', sent: false, request: summary, message: surface.run_request.confirmation_timeout };
        }
        if (outcome === 'unavailable') {
            return {
                status: 'confirmation_required',
                sent: false,
                request: summary,
                confirmation_token: issueConfirmation(fingerprint()),
                message: surface.run_request.confirmation_required
            };
        }
    }
    const [result] = (await sendRequest(scope, options, prepared)).results;
    const body = result.body.length > MAX_RESPONSE_BODY_LENGTH ? result.body.slice(0, MAX_RESPONSE_BODY_LENGTH) : result.body;
    return {
        status: 'sent',
        sent: true,
        request: { ...summary, headers: redactHeaders(result.request_headers, prepared.secret_values) },
        response: {
            status: result.status,
            elapsed_ms: result.elapsed_ms,
            headers: redactHeaders(result.response_headers, prepared.secret_values),
            body,
            body_truncated: body.length < result.body.length
        }
    };
}

async function fileInside(workspace: string, file: string): Promise<string> {
    const [root, target] = await Promise.all([realPath(workspace), realPath(path.resolve(workspace, file))]);
    if (!isInside(root, target)) {
        throw new Error(surface.run_request.file_outside_workspace.replace('{file}', file).replace('{workspace}', workspace));
    }
    return posix(target);
}

async function realPath(target: string): Promise<string> {
    try {
        return await fs.promises.realpath(target);
    } catch (err) {
        throw accessError(target, err);
    }
}

function rejectUndeclaredVariables(variables: Record<string, string> | undefined, required: string[]): void {
    const allowed = new Set([...required, AUTH_TOKEN_VARIABLE]);
    const undeclared = Object.keys(variables ?? {}).filter(name => !allowed.has(name));
    if (undeclared.length > 0) {
        throw new Error(surface.run_request.undeclared_variables
            .replace('{variables}', undeclared.map(name => `\`${name}\``).join(', '))
            .replace('{required}', required.length > 0 ? required.map(name => `\`${name}\``).join(', ') : 'none'));
    }
}

function summaryOf(prepared: PreparedRequest, environment?: string): RequestSummary {
    const url = redactSecrets(prepared.url, prepared.secret_values);
    return { request: prepared.name, method: prepared.method, url, environment: environment ?? null };
}

function cancellationOf(signal?: AbortSignal): CancellationSignal | undefined {
    if (!signal) {
        return undefined;
    }
    return {
        get isCancellationRequested() { return signal.aborted; },
        onCancellationRequested(listener: () => void) {
            signal.addEventListener('abort', listener);
            return { dispose: () => signal.removeEventListener('abort', listener) };
        }
    };
}

function fingerprintOf(args: RunRequestArgs, workspace: string, definition: unknown): string {
    const variables = Object.entries(args.variables ?? {}).sort(([a], [b]) => a.localeCompare(b));
    return canonicalJson([args.name, args.file ?? null, workspace, args.env ?? null, variables, definition]);
}

function canonicalJson(value: unknown): string {
    if (Array.isArray(value)) {
        return `[${value.map(canonicalJson).join(',')}]`;
    }
    if (value !== null && typeof value === 'object') {
        const entries = Object.entries(value).sort(([a], [b]) => a.localeCompare(b));
        return `{${entries.map(([key, entry]) => `${JSON.stringify(key)}:${canonicalJson(entry)}`).join(',')}}`;
    }
    return JSON.stringify(value) ?? 'null';
}

function issueConfirmation(fingerprint: string): string {
    const now = Date.now();
    for (const [token, pending] of pendingConfirmations) {
        if (pending.expiresAt < now) {
            pendingConfirmations.delete(token);
        }
    }
    const token = crypto.randomUUID();
    pendingConfirmations.set(token, { fingerprint, expiresAt: now + CONFIRMATION_TTL_MS });
    return token;
}

function redeemConfirmation(token: string, fingerprint: string): ConfirmationOutcome {
    const pending = pendingConfirmations.get(token);
    pendingConfirmations.delete(token);
    if (!pending || pending.expiresAt < Date.now() || pending.fingerprint !== fingerprint) {
        throw new Error(surface.run_request.invalid_confirmation_token);
    }
    return 'accepted';
}

function redactHeaders(headers: Record<string, string>, secrets: string[]): Record<string, string> {
    return Object.fromEntries(
        Object.entries(headers).map(([name, value]) => [name, SENSITIVE_HEADERS.has(name.toLowerCase()) ? REDACTED : redactSecrets(value, secrets)])
    );
}

function redactSecrets(text: string, secrets: string[]): string {
    return secrets.reduce((redacted, secret) => redacted.split(secret).join(REDACTED), text);
}
