import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import { setWasmTransport, wasmCall } from '../wasmHost';
import { buildFilesMap, buildSecretsMap, DraftFile, isDirectory, normalizePath } from '../utils';
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

async function filesFor(workspace: string, draft: DraftFile): Promise<string> {
    if (isUnscoped(workspace)) {
        return JSON.stringify({ [draft.path]: draft.source });
    }
    return buildFilesMap(workspace, [draft]);
}

export async function validateRq(args: {
    source: string;
    path?: string;
    workspace_path?: string;
    env?: string;
}): Promise<CheckResult> {
    const workspace = await workspaceFor(args.workspace_path, args.path);
    const draft = draftPath(workspace, args.path);
    const filesJson = await filesFor(workspace, { path: draft, source: args.source });
    const secretsJson = await buildSecretsMap(workspace);
    const raw = await wasmCall('check', [filesJson, secretsJson, draft, args.env]);
    const parsed = JSON.parse(raw) as { errors: Diagnostic[] };
    const diagnostics = (parsed.errors ?? []).map(e => ({ ...e, severity: e.severity ?? 'error' }));
    return { ok: diagnostics.length === 0, diagnostics };
}

export async function lintRq(args: { source: string; path?: string; workspace_path?: string }): Promise<CheckResult> {
    const workspace = await workspaceFor(args.workspace_path, args.path);
    const draft = draftPath(workspace, args.path);
    const filesJson = await filesFor(workspace, { path: draft, source: args.source });
    return JSON.parse(await wasmCall('lint', [filesJson, args.source, draft])) as CheckResult;
}

export async function listRequests(args: { path?: string }): Promise<unknown> {
    const target = args.path ? posix(await existing(args.path)) : await workspaceFor();
    if (isUnscoped(target)) {
        throw new Error(`Refusing to scan every file under ${target}: pass the project folder as \`path\`.`);
    }
    const filesJson = await buildFilesMap(target);
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
