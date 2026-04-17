import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import * as http from 'http';
import * as https from 'https';
import * as crypto from 'crypto';
import { execFileSync } from 'child_process';
import { normalizePath, collectAllFiles } from './utils';

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

export interface ReferenceLocation {
    file: string;
    line: number;
    character: number;
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
    endpoint: string | null;
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

export interface RequestShowOutput {
    name: string;
    auth?: {
        name: string;
        type: string;
    };
    file: string;
    line: number;
    character: number;
}

export interface LocationOutput {
    file: string;
    line: number;
    character: number;
}

export interface ExecuteRequestOptions {
    requestName: string;
    sourceDirectory?: string;
    environment?: string;
    variables?: Record<string, string>;
}

export interface RequestExecutionResult {
    request_name: string;
    method: string;
    url: string;
    status: number;
    elapsed_ms: number;
    request_headers: Record<string, string>;
    response_headers: Record<string, string>;
    body: string;
}

export interface ExecuteRequestResult {
    results: RequestExecutionResult[];
    stderr?: string;
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
// WASM module
// ---------------------------------------------------------------------------

interface RqWasmModule {
    list_requests(files_json: string, secrets_json: string, source: string): string;
    list_auth(files_json: string, secrets_json: string, source: string): string;
    list_environments(files_json: string, secrets_json: string, source: string): string;
    list_endpoints(files_json: string, secrets_json: string, source: string): string;
    list_variables(files_json: string, secrets_json: string, source: string, env: string | undefined): string;
    check(files_json: string, secrets_json: string, source: string, env: string | undefined): string;
    get_request_details(files_json: string, secrets_json: string, source: string, name: string, env: string | undefined, interpolate: boolean): string;
    get_auth_details(files_json: string, secrets_json: string, source: string, name: string, env: string | undefined, interpolate: boolean): string;
    get_environment(files_json: string, secrets_json: string, source: string, name: string): string;
    get_endpoint(files_json: string, secrets_json: string, source: string, name: string): string;
    get_variable(files_json: string, secrets_json: string, source: string, name: string, env: string | undefined, interpolate: boolean): string;
    list_variable_refs(files_json: string, secrets_json: string, source: string, name: string): string;
    list_endpoint_refs(files_json: string, secrets_json: string, source: string, name: string): string;
    version(): string;
    run_request(files_json: string, secrets_json: string, source: string, request_name: string, env: string | undefined, variables_json: string | undefined): Promise<string>;
}

let _wasm: RqWasmModule | null = null;

function getWasm(): RqWasmModule {
    if (!_wasm) {
        // eslint-disable-next-line @typescript-eslint/no-require-imports
        _wasm = require('./wasm/rq_wasm') as RqWasmModule;
    }
    return _wasm;
}

// ---------------------------------------------------------------------------
// File and secrets helpers
// ---------------------------------------------------------------------------

function resolveSource(sourceDir?: string): string {
    const raw = sourceDir ?? vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? process.cwd();
    return raw.replace(/\\/g, '/');
}

function getSourceDir(source: string): string {
    try {
        return fs.statSync(source).isDirectory() ? source : path.dirname(source);
    } catch {
        return source;
    }
}

function buildFilesMap(source: string): string {
    const dir = getSourceDir(source);
    const files: Record<string, string> = {};
    for (const filePath of collectAllFiles(dir)) {
        const normalized = filePath.replace(/\\/g, '/');
        try {
            files[normalized] = fs.readFileSync(filePath, 'utf8');
        } catch {
            // skip unreadable files
        }
    }
    return JSON.stringify(files);
}

function buildSecretsMap(source: string): string {
    const dir = getSourceDir(source);

    let envFile: string | null = null;
    try {
        envFile = fs.readFileSync(path.join(dir, '.env'), 'utf8');
    } catch {
        // no .env file
    }

    const osVars: [string, string][] = [];
    for (const [key, value] of Object.entries(process.env)) {
        if (key.startsWith('RQ__') && value !== undefined) {
            osVars.push([key, value]);
        }
    }

    return JSON.stringify({ env_file: envFile, os_vars: osVars });
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
    Auth?: { name: string; type: string };
    file: string;
    line: number;
    character: number;
}

interface EnvironmentEntry {
    name: string;
    file: string;
    line: number;
    character: number;
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

export async function listEnvironments(sourceDirectory?: string): Promise<string[]> {
    const source = resolveSource(sourceDirectory);
    const result = getWasm().list_environments(buildFilesMap(source), buildSecretsMap(source), source);
    const entries = JSON.parse(result) as EnvironmentEntry[];
    return entries.map(e => e.name);
}

export async function listAuthConfigs(sourceDirectory?: string): Promise<AuthListEntry[]> {
    const source = resolveSource(sourceDirectory);
    const result = getWasm().list_auth(buildFilesMap(source), buildSecretsMap(source), source);
    return JSON.parse(result) as AuthListEntry[];
}

export async function showEnvironment(name: string, sourceDirectory?: string): Promise<EnvironmentShowOutput> {
    const source = resolveSource(sourceDirectory);
    const result = getWasm().get_environment(buildFilesMap(source), buildSecretsMap(source), source, name);
    const raw = JSON.parse(result) as EnvironmentShowOutput;
    return { ...raw, file: normalizePath(raw.file) };
}

export async function listEndpoints(sourceDirectory?: string): Promise<EndpointShowOutput[]> {
    const source = resolveSource(sourceDirectory);
    const result = getWasm().list_endpoints(buildFilesMap(source), buildSecretsMap(source), source);
    const raw = JSON.parse(result) as EndpointShowOutput[];
    return raw.map(e => ({ ...e, file: normalizePath(e.file) }));
}

export async function showEndpoint(name: string, sourceDirectory?: string): Promise<EndpointShowOutput> {
    const source = resolveSource(sourceDirectory);
    const result = getWasm().get_endpoint(buildFilesMap(source), buildSecretsMap(source), source, name);
    const raw = JSON.parse(result) as EndpointShowOutput;
    return { ...raw, file: normalizePath(raw.file) };
}

export async function showVariable(name: string, sourceDirectory?: string, environment?: string, interpolateVariables = true): Promise<VariableShowOutput> {
    const source = resolveSource(sourceDirectory);
    const result = getWasm().get_variable(buildFilesMap(source), buildSecretsMap(source), source, name, environment, interpolateVariables);
    const raw = JSON.parse(result) as VariableShowOutput;
    return { ...raw, file: normalizePath(raw.file) };
}

export async function listVariables(sourceFile?: string, environment?: string): Promise<VariableShowOutput[]> {
    const source = resolveSource(sourceFile);
    const result = getWasm().list_variables(buildFilesMap(source), buildSecretsMap(source), source, environment);
    const raw = JSON.parse(result) as VariableShowOutput[];
    return raw.map(v => ({ ...v, file: normalizePath(v.file) }));
}

export async function varRefs(name: string, sourceDirectory?: string): Promise<ReferenceLocation[]> {
    const source = resolveSource(sourceDirectory);
    const result = getWasm().list_variable_refs(buildFilesMap(source), buildSecretsMap(source), source, name);
    const raw = JSON.parse(result) as ReferenceLocation[];
    return raw.map(r => ({ ...r, file: normalizePath(r.file) }));
}

export async function epRefs(name: string, sourceDirectory?: string): Promise<ReferenceLocation[]> {
    const source = resolveSource(sourceDirectory);
    const result = getWasm().list_endpoint_refs(buildFilesMap(source), buildSecretsMap(source), source, name);
    const raw = JSON.parse(result) as ReferenceLocation[];
    return raw.map(r => ({ ...r, file: normalizePath(r.file) }));
}

export async function showAuthConfig(name: string, sourceDirectory?: string, environment?: string): Promise<AuthShowOutput> {
    const source = resolveSource(sourceDirectory);
    const result = getWasm().get_auth_details(buildFilesMap(source), buildSecretsMap(source), source, name, environment, true);
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
    const result = getWasm().list_requests(buildFilesMap(source), buildSecretsMap(source), source);
    const requests = JSON.parse(result) as RequestInfo[];
    requests.forEach(r => {
        r.file = normalizePath(r.file);
        if (r.endpoint_file) { r.endpoint_file = normalizePath(r.endpoint_file); }
    });
    return { requests };
}

export async function showRequest(requestName: string, sourceDirectory?: string, environment?: string, interpolate = false): Promise<RequestShowOutput> {
    const source = resolveSource(sourceDirectory);
    const result = getWasm().get_request_details(buildFilesMap(source), buildSecretsMap(source), source, requestName, environment, interpolate);
    const raw = JSON.parse(result) as RequestShowRaw;
    return {
        name: raw.Request,
        auth: raw.Auth,
        file: normalizePath(raw.file),
        line: raw.line,
        character: raw.character,
    };
}

export async function showAuthLocation(name: string, sourceDirectory?: string): Promise<LocationOutput> {
    const source = resolveSource(sourceDirectory);
    const result = getWasm().get_auth_details(buildFilesMap(source), buildSecretsMap(source), source, name, undefined, false);
    const raw = JSON.parse(result) as AuthShowRaw;
    return { file: normalizePath(raw.file), line: raw.line, character: raw.character };
}

export async function showRequestLocation(requestName: string, sourceDirectory?: string): Promise<LocationOutput> {
    const source = resolveSource(sourceDirectory);
    const result = getWasm().get_request_details(buildFilesMap(source), buildSecretsMap(source), source, requestName, undefined, false);
    const raw = JSON.parse(result) as RequestShowRaw;
    return { file: normalizePath(raw.file), line: raw.line, character: raw.character };
}

export async function checkFolder(folderPath: string, envName?: string): Promise<CheckResult> {
    const result = getWasm().check(buildFilesMap(folderPath), buildSecretsMap(folderPath), folderPath, envName);
    return JSON.parse(result) as CheckResult;
}

export async function executeRequest(options: ExecuteRequestOptions): Promise<ExecuteRequestResult> {
    const source = resolveSource(options.sourceDirectory);
    const filesJson = buildFilesMap(source);
    const secretsJson = buildSecretsMap(source);

    const detailsRaw = getWasm().get_request_details(
        filesJson, secretsJson, source,
        options.requestName, options.environment, true,
    );
    const raw = JSON.parse(detailsRaw) as RequestShowRaw;

    const url = raw.URL;
    const method = raw.Method;
    const body = raw.Body;
    const headers: Record<string, string> = { ...raw.Headers };

    if (raw.Auth) {
        const { name: authName, type: authType } = raw.Auth;
        if (authType === 'oauth2_authorization_code' || authType === 'oauth2_implicit') {
            const token = options.variables?.['auth_token'];
            if (token) {
                headers['authorization'] = `Bearer ${token}`;
            }
        } else if (authType === 'bearer') {
            const authDetails = await showAuthConfig(authName, options.sourceDirectory, options.environment);
            const token = authDetails.fields['token'];
            if (token) {
                headers['authorization'] = `Bearer ${token}`;
            }
        } else if (authType === 'oauth2_client_credentials') {
            const authDetails = await showAuthConfig(authName, options.sourceDirectory, options.environment);
            const token = await fetchClientCredentialsToken(authDetails.fields, path.dirname(authDetails.file));
            headers['authorization'] = `Bearer ${token}`;
        }
    }

    const startTime = Date.now();
    const response = await nodeHttpRequest(url, method, headers, body);
    const elapsed = Date.now() - startTime;

    return {
        results: [{
            request_name: options.requestName,
            method,
            url,
            status: response.status,
            elapsed_ms: elapsed,
            request_headers: headers,
            response_headers: response.headers,
            body: response.body,
        }],
    };
}

interface NodeHttpResponse {
    status: number;
    headers: Record<string, string>;
    body: string;
}

function nodeHttpRequest(url: string, method: string, reqHeaders: Record<string, string>, body?: string): Promise<NodeHttpResponse> {
    return new Promise((resolve, reject) => {
        const parsed = new URL(url);
        const isHttps = parsed.protocol === 'https:';
        const mod = isHttps ? https : http;

        const outHeaders: Record<string, string> = { ...reqHeaders };
        if (body) {
            outHeaders['content-length'] = String(Buffer.byteLength(body, 'utf8'));
        }

        const req = mod.request({
            hostname: parsed.hostname,
            port: parsed.port || (isHttps ? 443 : 80),
            path: parsed.pathname + parsed.search,
            method,
            headers: outHeaders,
        }, (res) => {
            const responseHeaders: Record<string, string> = {};
            for (const [k, v] of Object.entries(res.headers)) {
                if (v !== undefined) {
                    responseHeaders[k] = Array.isArray(v) ? v.join(', ') : v;
                }
            }
            const chunks: Buffer[] = [];
            res.on('data', (chunk: Buffer) => chunks.push(chunk));
            res.on('end', () => resolve({
                status: res.statusCode ?? 0,
                headers: responseHeaders,
                body: Buffer.concat(chunks).toString('utf8'),
            }));
            res.on('error', reject);
        });

        req.on('error', reject);
        if (body) { req.write(body, 'utf8'); }
        req.end();
    });
}

async function fetchClientCredentialsToken(fields: Record<string, string>, authFileDir?: string): Promise<string> {
    const { client_id, client_secret, token_url, scope, cert_file, cert_password } = fields;

    if (cert_file) {
        const certPath = (authFileDir && !path.isAbsolute(cert_file))
            ? path.join(authFileDir, cert_file)
            : cert_file;
        const certContent = fs.readFileSync(certPath);
        const { certDer, privateKeyPem } = certContent.toString('utf8').includes('-----BEGIN')
            ? parsePemCert(certContent)
            : extractPemFromP12(certPath, cert_password ?? '');
        const assertion = createJwtAssertion(privateKeyPem, certDer, client_id, token_url);
        const params = new URLSearchParams({
            grant_type: 'client_credentials',
            client_id,
            client_assertion_type: 'urn:ietf:params:oauth:client-assertion-type:jwt-bearer',
            client_assertion: assertion,
        });
        if (scope) { params.set('scope', scope); }
        const response = await nodeHttpRequest(token_url, 'POST', {
            'content-type': 'application/x-www-form-urlencoded',
        }, params.toString());
        if (response.status < 200 || response.status >= 300) {
            throw new Error(`Token request failed with status ${response.status}: ${response.body}`);
        }
        const data = JSON.parse(response.body) as { access_token?: string };
        if (!data.access_token) {
            throw new Error(`Token response missing access_token: ${response.body}`);
        }
        return data.access_token;
    }

    const params = new URLSearchParams({ grant_type: 'client_credentials', client_id, client_secret });
    if (scope) { params.set('scope', scope); }
    const response = await nodeHttpRequest(token_url, 'POST', {
        'content-type': 'application/x-www-form-urlencoded',
    }, params.toString());
    if (response.status < 200 || response.status >= 300) {
        throw new Error(`Token request failed with status ${response.status}: ${response.body}`);
    }
    const data = JSON.parse(response.body) as { access_token?: string };
    if (!data.access_token) {
        throw new Error(`Token response missing access_token: ${response.body}`);
    }
    return data.access_token;
}

export function base64url(buf: Buffer): string {
    return buf.toString('base64').replace(/\+/g, '-').replace(/\//g, '_').replace(/=/g, '');
}

export function parsePemCert(content: Buffer): { certDer: Buffer; privateKeyPem: Buffer } {
    const text = content.toString('utf8');
    const certMatch = text.match(/-----BEGIN CERTIFICATE-----[\s\S]+?-----END CERTIFICATE-----/);
    if (!certMatch) { throw new Error('No CERTIFICATE block found in PEM file'); }
    const certDer = Buffer.from(
        certMatch[0].split('\n').filter(l => !l.startsWith('-----')).join(''),
        'base64',
    );
    const keyMatch = text.match(/-----BEGIN (?:[A-Z]+ )?PRIVATE KEY-----[\s\S]+?-----END (?:[A-Z]+ )?PRIVATE KEY-----/);
    if (!keyMatch) { throw new Error('No PRIVATE KEY block found in PEM file. Ensure the file contains an unencrypted private key.'); }
    return { certDer, privateKeyPem: Buffer.from(keyMatch[0]) };
}

export function extractPemFromP12(p12Path: string, password: string): { certDer: Buffer; privateKeyPem: Buffer } {
    const baseArgs = ['-in', p12Path, '-passin', `pass:${password}`];
    const run = (extra: string[]) => execFileSync('openssl', ['pkcs12', ...baseArgs, ...extra]);
    try {
        const combined = Buffer.concat([run(['-nokeys']), run(['-nocerts', '-nodes'])]);
        return parsePemCert(combined);
    } catch {
        try {
            const combined = Buffer.concat([run(['-nokeys', '-legacy']), run(['-nocerts', '-nodes', '-legacy'])]);
            return parsePemCert(combined);
        } catch (e: unknown) {
            const msg = e instanceof Error ? e.message : String(e);
            throw new Error(`Failed to parse .p12 certificate. Ensure openssl is installed and the password is correct. ${msg}`);
        }
    }
}

export function createJwtAssertion(privateKeyPem: Buffer, certDer: Buffer, clientId: string, tokenUrl: string): string {
    const now = Math.floor(Date.now() / 1000);
    const x5t = base64url(crypto.createHash('sha1').update(certDer).digest());
    const header = base64url(Buffer.from(JSON.stringify({ alg: 'RS256', typ: 'JWT', x5t })));
    const payload = base64url(Buffer.from(JSON.stringify({
        iss: clientId,
        sub: clientId,
        aud: tokenUrl,
        jti: crypto.randomUUID(),
        iat: now,
        nbf: now - 60,
        exp: now + 300,
    })));
    const signingInput = `${header}.${payload}`;
    const sign = crypto.createSign('RSA-SHA256');
    sign.update(signingInput);
    return `${signingInput}.${base64url(sign.sign(privateKeyPem))}`;
}
