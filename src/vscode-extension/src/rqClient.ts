import * as vscode from 'vscode';
import * as path from 'path';
import { normalizePath, draftsJson, buildSecretsMap, DraftFile, isInsideFolder } from './utils';
import * as execution from './execution';
import { ExecuteRequestOptions, ExecuteRequestResult, requestScope, resolveScopeFile } from './execution';
import { wasmCall } from './wasmHost';
import { UiColor } from './language/definitions';

export type { CancellationSignal, ExecuteRequestOptions, ExecuteRequestResult, PreparedAuth, PreparedRequest, RequestExecutionResult } from './execution';

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

export interface Environment {
    name: string;
}

export interface EnvironmentShowOutput {
    name: string;
    file: string;
    line: number;
    character: number;
}

export interface EndpointShowOutput {
    name: string;
    file: string;
    line: number;
    character: number;
    is_template?: boolean;
}

export interface VariableShowOutput {
    name: string;
    value: string;
    file: string;
    line: number;
    character: number;
    source: string;
}

interface ReferenceLocation {
    file: string;
    line: number;
    character: number;
    shared?: boolean;
}

export interface SymbolReference {
    uri: vscode.Uri;
    line: number;
    character: number;
    shared: boolean;
}

export interface AuthConfig {
    name: string;
}

export interface AuthListEntry {
    name: string;
    auth_type: string;
}

export type AuthListOutput = AuthListEntry[];

export interface AuthShowOutput {
    name: string;
    auth_type: string;
    fields: Record<string, string>;
    environment?: string;
    file: string;
    line: number;
    character: number;
}

export interface RequestInfo {
    name: string;
    method?: string;
    url?: string;
    description?: string | null;
    endpoint: string | null;
    endpoint_description?: string | null;
    file: string;
    endpoint_file?: string;
    endpoint_line?: number;
    endpoint_character?: number;
}

export type RequestListOutput = RequestInfo[];

export interface ListRequestsResult {
    requests: RequestInfo[];
    errors?: string[];
}

export interface ListEnvironmentsResult {
    environments: string[];
    colors: Map<string, UiColor>;
    errors: string[];
}

export interface ListAuthConfigsResult {
    authConfigs: AuthListEntry[];
    errors: string[];
}

export interface RequestShowOutput {
    name: string;
    method: string;
    url: string;
    headers: Record<string, string>;
    auth?: {
        name: string;
        type: string;
    };
    requiredVariables: string[];
    file: string;
    line: number;
    character: number;
}

export interface LocationOutput {
    file: string;
    line: number;
    character: number;
}

export interface CheckDiagnostic {
    file: string;
    line: number;
    column: number;
    message: string;
}

export interface CheckResult {
    errors: CheckDiagnostic[];
}

// ---------------------------------------------------------------------------
// File and secrets helpers
// ---------------------------------------------------------------------------

function resolveSource(sourceDir?: string): string {
    const raw = sourceDir ?? vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? process.cwd();
    return raw.replace(/\\/g, '/');
}

function unsavedDrafts(folder: string): DraftFile[] {
    return vscode.workspace.textDocuments
        .filter(doc => doc.languageId === 'rq' && doc.isDirty && !doc.isUntitled && isInsideFolder(doc.uri.fsPath, folder))
        .map(doc => ({ path: forwardSlashes(doc.uri.fsPath), source: doc.getText() }));
}

// ---------------------------------------------------------------------------
// Raw output shapes returned by WASM bindings
// ---------------------------------------------------------------------------

interface AuthShowRaw {
    'Auth Configuration': string;
    Type: string;
    Fields: Record<string, string>;
    Environment?: string;
    file: string;
    line: number;
    character: number;
}

interface RequestShowRaw {
    Request: string;
    URL: string;
    Method: string;
    Headers: Record<string, string>;
    Body?: string;
    Timeout?: string;
    Auth?: { name: string; type: string };
    RequiredVariables?: string[];
    file: string;
    line: number;
    character: number;
}

interface EnvironmentEntry {
    name: string;
    file: string;
    line: number;
    character: number;
    color: UiColor | null;
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

export async function listEnvironments(sourceDirectory?: string): Promise<string[]> {
    return (await listEnvironmentsWithErrors(sourceDirectory)).environments;
}

export async function listEnvironmentsWithErrors(sourceDirectory?: string): Promise<ListEnvironmentsResult> {
    const source = resolveSource(sourceDirectory);
    const result = await wasmCall('list_environments', [draftsJson(), await buildSecretsMap(source), source]);
    const parsed = JSON.parse(result) as { environments: EnvironmentEntry[]; parse_errors?: { message: string }[] };
    return {
        environments: parsed.environments.map(e => e.name),
        colors: new Map(parsed.environments.flatMap(e => e.color ? [[e.name, e.color] as const] : [])),
        errors: (parsed.parse_errors ?? []).map(e => e.message),
    };
}

export interface AuthField {
    name: string;
    required: boolean;
}

let authFieldsByType: Promise<Record<string, AuthField[]>> | undefined;

export async function authFields(): Promise<Record<string, AuthField[]>> {
    authFieldsByType ??= loadAuthFields().catch(error => {
        authFieldsByType = undefined;
        throw error;
    });
    return authFieldsByType;
}

async function loadAuthFields(): Promise<Record<string, AuthField[]>> {
    const types = JSON.parse(await wasmCall('auth_fields', [])) as { auth_type: string; fields: AuthField[] }[];
    return Object.fromEntries(types.map(entry => [entry.auth_type, entry.fields]));
}

export async function listAuthConfigs(sourceDirectory?: string, rootDirectory?: string, drafts: DraftFile[] = []): Promise<AuthListEntry[]> {
    return (await listAuthConfigsWithErrors(sourceDirectory, rootDirectory, drafts)).authConfigs;
}

export async function listAuthConfigsWithErrors(sourceDirectory?: string, rootDirectory?: string, drafts: DraftFile[] = []): Promise<ListAuthConfigsResult> {
    const source = resolveSource(sourceDirectory);
    const root = resolveSource(rootDirectory ?? sourceDirectory);
    const result = await wasmCall('list_auth', [draftsJson(drafts), await buildSecretsMap(root), source]);
    const parsed = JSON.parse(result) as { auth: AuthListEntry[]; parse_errors?: { message: string }[] };
    return {
        authConfigs: parsed.auth,
        errors: (parsed.parse_errors ?? []).map(e => e.message),
    };
}

export async function showEnvironment(name: string, sourceDirectory?: string): Promise<EnvironmentShowOutput> {
    const source = resolveSource(sourceDirectory);
    const result = await wasmCall('get_environment', [draftsJson(), await buildSecretsMap(source), source, name]);
    const raw = JSON.parse(result) as EnvironmentShowOutput;
    return { ...raw, file: normalizePath(raw.file) };
}

export async function listEndpoints(sourceDirectory?: string, rootDirectory?: string, drafts: DraftFile[] = []): Promise<EndpointShowOutput[]> {
    const source = resolveSource(sourceDirectory);
    const root = resolveSource(rootDirectory ?? sourceDirectory);
    const result = await wasmCall('list_endpoints', [draftsJson(drafts), await buildSecretsMap(root), source]);
    const raw = JSON.parse(result) as EndpointShowOutput[];
    return raw.map(e => ({ ...e, file: normalizePath(e.file) }));
}

export async function showEndpoint(name: string, sourceDirectory?: string, scopeFile?: string): Promise<EndpointShowOutput> {
    const source = resolveSource(sourceDirectory);
    const result = await wasmCall('get_endpoint', [draftsJson(), await buildSecretsMap(source), source, name, resolveScopeFile(scopeFile)]);
    const raw = JSON.parse(result) as EndpointShowOutput;
    return { ...raw, file: normalizePath(raw.file) };
}

export async function showVariable(name: string, sourceDirectory?: string, environment?: string, interpolateVariables = true, scopeFile?: string): Promise<VariableShowOutput> {
    const source = resolveSource(sourceDirectory);
    const result = await wasmCall('get_variable', [draftsJson(), await buildSecretsMap(source), source, name, environment, interpolateVariables, resolveScopeFile(scopeFile)]);
    const raw = JSON.parse(result) as VariableShowOutput;
    return { ...raw, file: normalizePath(raw.file) };
}

export async function listVariables(sourceFile?: string, environment?: string, rootDirectory?: string, drafts: DraftFile[] = []): Promise<VariableShowOutput[]> {
    const source = resolveSource(sourceFile);
    const root = resolveSource(rootDirectory ?? sourceFile);
    const result = await wasmCall('list_variables', [draftsJson(drafts), await buildSecretsMap(root), source, environment]);
    const raw = JSON.parse(result) as VariableShowOutput[];
    return raw.map(v => ({ ...v, file: normalizePath(v.file) }));
}

export async function varRefs(name: string, document: vscode.TextDocument, position: vscode.Position, sourceDirectory?: string): Promise<SymbolReference[]> {
    return symbolRefs('list_variable_refs', name, document, sourceDirectory, [position.line, position.character]);
}

export async function epRefs(name: string, document: vscode.TextDocument, sourceDirectory?: string): Promise<SymbolReference[]> {
    return symbolRefs('list_endpoint_refs', name, document, sourceDirectory, []);
}

async function symbolRefs(binding: 'list_variable_refs' | 'list_endpoint_refs', name: string, document: vscode.TextDocument, sourceDirectory: string | undefined, extraArgs: unknown[]): Promise<SymbolReference[]> {
    const source = resolveSource(sourceDirectory);
    const scopeFile = documentDraftPath(document, source);
    const filesJson = draftsJson(openDrafts(document, source));
    const secretsJson = await buildSecretsMap(source);
    const result = await wasmCall(binding, [filesJson, secretsJson, source, name, scopeFile, ...extraArgs]);
    const raw = JSON.parse(result) as ReferenceLocation[];
    return raw.map(r => ({
        uri: sameFile(r.file, scopeFile) ? document.uri : vscode.Uri.file(normalizePath(r.file)),
        line: r.line,
        character: r.character,
        shared: r.shared ?? false,
    }));
}

function documentDraftPath(document: vscode.TextDocument, source: string): string {
    return forwardSlashes(document.isUntitled ? path.join(source, document.uri.path) : document.uri.fsPath);
}

function sameFile(a: string, b: string): boolean {
    return forwardSlashes(normalizePath(a)) === forwardSlashes(normalizePath(b));
}

function forwardSlashes(filePath: string): string {
    return filePath.replace(/\\/g, '/');
}

function openDrafts(document: vscode.TextDocument, source: string): DraftFile[] {
    const current = documentDraftPath(document, source);
    const dirty = unsavedDrafts(source).filter(draft => draft.path !== current);
    return [{ path: current, source: document.getText() }, ...dirty];
}

export async function showAuthConfig(name: string, sourceDirectory?: string, environment?: string): Promise<AuthShowOutput> {
    const source = resolveSource(sourceDirectory);
    const result = await wasmCall('get_auth_details', [draftsJson(), await buildSecretsMap(source), source, name, environment, true]);
    const raw = JSON.parse(result) as AuthShowRaw;
    return {
        name: raw['Auth Configuration'],
        auth_type: raw.Type,
        fields: raw.Fields,
        environment: raw.Environment,
        file: normalizePath(raw.file),
        line: raw.line,
        character: raw.character,
    };
}

export async function listRequests(sourceDirectory?: string): Promise<ListRequestsResult> {
    const source = resolveSource(sourceDirectory);
    const result = await wasmCall('list_requests', [draftsJson(), await buildSecretsMap(source), source]);
    const parsed = JSON.parse(result) as { requests: RequestInfo[]; parse_errors?: { message: string }[] };
    const requests = parsed.requests ?? [];
    requests.forEach(r => {
        r.file = normalizePath(r.file);
        if (r.endpoint_file) { r.endpoint_file = normalizePath(r.endpoint_file); }
    });
    return { requests, errors: (parsed.parse_errors ?? []).map(e => e.message) };
}

export async function showRequest(requestName: string, sourceDirectory?: string, environment?: string, interpolate = false, skipRequiredVariables = false, sourceFile?: string): Promise<RequestShowOutput> {
    const source = resolveSource(sourceDirectory);
    const { filesJson, secretsJson } = await requestScope(source);
    const result = await wasmCall('get_request_details', [filesJson, secretsJson, source, requestName, environment, interpolate, skipRequiredVariables, undefined, resolveScopeFile(sourceFile)]);
    const raw = JSON.parse(result) as RequestShowRaw;
    return {
        name: raw.Request,
        method: raw.Method,
        url: raw.URL,
        headers: raw.Headers,
        auth: raw.Auth,
        requiredVariables: raw.RequiredVariables ?? [],
        file: normalizePath(raw.file),
        line: raw.line,
        character: raw.character,
    };
}

export async function showAuthLocation(name: string, sourceDirectory?: string): Promise<LocationOutput> {
    const source = resolveSource(sourceDirectory);
    const result = await wasmCall('get_auth_details', [draftsJson(), await buildSecretsMap(source), source, name, undefined, false]);
    const raw = JSON.parse(result) as AuthShowRaw;
    return { file: normalizePath(raw.file), line: raw.line, character: raw.character };
}

export async function showRequestLocation(requestName: string, sourceDirectory?: string): Promise<LocationOutput> {
    const source = resolveSource(sourceDirectory);
    const result = await wasmCall('get_request_details', [draftsJson(), await buildSecretsMap(source), source, requestName, undefined, false, false]);
    const raw = JSON.parse(result) as RequestShowRaw;
    return { file: normalizePath(raw.file), line: raw.line, character: raw.character };
}

export interface LintDiagnostic {
    severity: string;
    rule: string;
    message: string;
    line: number;
    column: number;
    file?: string;
    suggested_fix?: string;
}

export interface LintResult {
    ok: boolean;
    diagnostics: LintDiagnostic[];
}

function draftKey(filePath: string): string {
    return normalizePath(filePath).replace(/\\/g, '/');
}

export async function lintSources(files: DraftFile[], workspaceDirectory?: string): Promise<Map<string, LintResult>> {
    const results = new Map<string, LintResult>();
    if (files.length === 0) {
        return results;
    }

    const workspace = resolveSource(workspaceDirectory);
    const drafts = files.map(f => ({ path: draftKey(f.path), source: f.source }));
    const filesJson = draftsJson(drafts);
    const lint = async (lintWorkspace?: string) => JSON.parse(await wasmCall('lint', [filesJson, lintWorkspace])) as Record<string, LintResult>;
    const byPath = await lint(workspace).catch(() => lint(undefined));
    for (const draft of drafts) {
        results.set(draft.path, byPath[draft.path] ?? { ok: true, diagnostics: [] });
    }
    return results;
}

export async function lintSource(source: string, filePath: string, workspaceDirectory?: string): Promise<LintResult> {
    const results = await lintSources([{ path: filePath, source }], workspaceDirectory);
    return results.get(draftKey(filePath)) ?? { ok: true, diagnostics: [] };
}

export type SuppressionScope = 'line' | 'statement' | 'file';

export interface SourceEdit {
    start_line: number;
    start_column: number;
    end_line: number;
    end_column: number;
    new_text: string;
}

export async function suppressionEdit(source: string, rule: string, line: number, column: number, scope: SuppressionScope): Promise<SourceEdit | null> {
    const raw = await wasmCall('suppression_edit', [source, rule, line, column, scope]);
    return JSON.parse(raw) as SourceEdit | null;
}

export async function unusedSuppressionRemoval(source: string, line: number, column: number): Promise<SourceEdit | null> {
    const raw = await wasmCall('unused_suppression_removal', [source, line, column]);
    return JSON.parse(raw) as SourceEdit | null;
}

export async function checkFolder(folderPath: string, envName?: string, drafts: DraftFile[] = []): Promise<CheckResult> {
    const result = await wasmCall('check', [draftsJson(drafts), await buildSecretsMap(folderPath), folderPath, envName]);
    return JSON.parse(result) as CheckResult;
}

export async function setDebugLogging(enabled: boolean): Promise<void> {
    await wasmCall('set_debug_logging', [enabled]);
}

export async function executeRequest(options: ExecuteRequestOptions): Promise<ExecuteRequestResult> {
    const source = resolveSource(options.sourceDirectory);
    return execution.executeRequest(await requestScope(source), options);
}
