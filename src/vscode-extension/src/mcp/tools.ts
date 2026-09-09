import * as path from 'path';
import { setWasmTransport, wasmCall } from '../wasmHost';
import { buildFilesMap, buildSecretsMap, isDirectory, normalizePath } from '../utils';

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
    return posix(path.isAbsolute(name) ? name : path.join(workspacePath, name));
}

export async function workspaceFor(workspacePath?: string, filePath?: string): Promise<string> {
    if (workspacePath) { return posix(workspacePath); }
    if (filePath && path.isAbsolute(filePath)) {
        return posix((await isDirectory(filePath)) ? filePath : path.dirname(filePath));
    }
    return posix(process.cwd());
}

export async function validateRq(args: {
    source: string;
    path?: string;
    workspace_path?: string;
    env?: string;
}): Promise<CheckResult> {
    const workspace = await workspaceFor(args.workspace_path, args.path);
    const draft = draftPath(workspace, args.path);
    const filesJson = await buildFilesMap(workspace, [{ path: draft, source: args.source }]);
    const secretsJson = await buildSecretsMap(workspace);
    const raw = await wasmCall('check', [filesJson, secretsJson, draft, args.env]);
    const parsed = JSON.parse(raw) as { errors: Diagnostic[] };
    const diagnostics = (parsed.errors ?? []).map(e => ({ ...e, severity: e.severity ?? 'error' }));
    return { ok: diagnostics.length === 0, diagnostics };
}

export async function lintRq(args: { source: string; path?: string; workspace_path?: string }): Promise<CheckResult> {
    const workspace = await workspaceFor(args.workspace_path, args.path);
    const draft = draftPath(workspace, args.path);
    const filesJson = await buildFilesMap(workspace, [{ path: draft, source: args.source }]);
    return JSON.parse(await wasmCall('lint', [filesJson, args.source, draft])) as CheckResult;
}

export async function listRequests(args: { path?: string }): Promise<unknown> {
    const target = await workspaceFor(args.path);
    const filesJson = await buildFilesMap(target);
    const secretsJson = await buildSecretsMap(target);
    const raw = await wasmCall('list_requests', [filesJson, secretsJson, target]);
    const parsed = JSON.parse(raw) as { requests: unknown[]; parse_errors?: Diagnostic[] };
    return { requests: parsed.requests ?? [], parse_errors: parsed.parse_errors ?? [] };
}
