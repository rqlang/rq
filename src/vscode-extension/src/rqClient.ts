import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import * as http from 'http';
import * as https from 'https';
import * as crypto from 'crypto';
import * as forge from 'node-forge';
import { normalizePath, buildFilesMap, buildSecretsMap, DraftFile } from './utils';
import { wasmCall } from './wasmHost';

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
    request_body?: string;
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
// File and secrets helpers
// ---------------------------------------------------------------------------

function resolveSource(sourceDir?: string): string {
    const raw = sourceDir ?? vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? process.cwd();
    return raw.replace(/\\/g, '/');
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
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

export async function listEnvironments(sourceDirectory?: string): Promise<string[]> {
    const source = resolveSource(sourceDirectory);
    const result = await wasmCall('list_environments', [await buildFilesMap(source), await buildSecretsMap(source), source]);
    const entries = JSON.parse(result) as EnvironmentEntry[];
    return entries.map(e => e.name);
}

export async function listAuthConfigs(sourceDirectory?: string): Promise<AuthListEntry[]> {
    const source = resolveSource(sourceDirectory);
    const result = await wasmCall('list_auth', [await buildFilesMap(source), await buildSecretsMap(source), source]);
    return JSON.parse(result) as AuthListEntry[];
}

export async function showEnvironment(name: string, sourceDirectory?: string): Promise<EnvironmentShowOutput> {
    const source = resolveSource(sourceDirectory);
    const result = await wasmCall('get_environment', [await buildFilesMap(source), await buildSecretsMap(source), source, name]);
    const raw = JSON.parse(result) as EnvironmentShowOutput;
    return { ...raw, file: normalizePath(raw.file) };
}

export async function listEndpoints(sourceDirectory?: string): Promise<EndpointShowOutput[]> {
    const source = resolveSource(sourceDirectory);
    const result = await wasmCall('list_endpoints', [await buildFilesMap(source), await buildSecretsMap(source), source]);
    const raw = JSON.parse(result) as EndpointShowOutput[];
    return raw.map(e => ({ ...e, file: normalizePath(e.file) }));
}

export async function showEndpoint(name: string, sourceDirectory?: string): Promise<EndpointShowOutput> {
    const source = resolveSource(sourceDirectory);
    const result = await wasmCall('get_endpoint', [await buildFilesMap(source), await buildSecretsMap(source), source, name]);
    const raw = JSON.parse(result) as EndpointShowOutput;
    return { ...raw, file: normalizePath(raw.file) };
}

export async function showVariable(name: string, sourceDirectory?: string, environment?: string, interpolateVariables = true): Promise<VariableShowOutput> {
    const source = resolveSource(sourceDirectory);
    const result = await wasmCall('get_variable', [await buildFilesMap(source), await buildSecretsMap(source), source, name, environment, interpolateVariables]);
    const raw = JSON.parse(result) as VariableShowOutput;
    return { ...raw, file: normalizePath(raw.file) };
}

export async function listVariables(sourceFile?: string, environment?: string): Promise<VariableShowOutput[]> {
    const source = resolveSource(sourceFile);
    const result = await wasmCall('list_variables', [await buildFilesMap(source), await buildSecretsMap(source), source, environment]);
    const raw = JSON.parse(result) as VariableShowOutput[];
    return raw.map(v => ({ ...v, file: normalizePath(v.file) }));
}

export async function varRefs(name: string, sourceDirectory?: string): Promise<ReferenceLocation[]> {
    const source = resolveSource(sourceDirectory);
    const result = await wasmCall('list_variable_refs', [await buildFilesMap(source), await buildSecretsMap(source), source, name]);
    const raw = JSON.parse(result) as ReferenceLocation[];
    return raw.map(r => ({ ...r, file: normalizePath(r.file) }));
}

export async function epRefs(name: string, sourceDirectory?: string): Promise<ReferenceLocation[]> {
    const source = resolveSource(sourceDirectory);
    const result = await wasmCall('list_endpoint_refs', [await buildFilesMap(source), await buildSecretsMap(source), source, name]);
    const raw = JSON.parse(result) as ReferenceLocation[];
    return raw.map(r => ({ ...r, file: normalizePath(r.file) }));
}

export async function showAuthConfig(name: string, sourceDirectory?: string, environment?: string): Promise<AuthShowOutput> {
    const source = resolveSource(sourceDirectory);
    const result = await wasmCall('get_auth_details', [await buildFilesMap(source), await buildSecretsMap(source), source, name, environment, true]);
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
    const result = await wasmCall('list_requests', [await buildFilesMap(source), await buildSecretsMap(source), source]);
    const parsed = JSON.parse(result) as { requests: RequestInfo[]; parse_errors?: { message: string }[] };
    const requests = parsed.requests ?? [];
    requests.forEach(r => {
        r.file = normalizePath(r.file);
        if (r.endpoint_file) { r.endpoint_file = normalizePath(r.endpoint_file); }
    });
    return { requests, errors: (parsed.parse_errors ?? []).map(e => e.message) };
}

export async function showRequest(requestName: string, sourceDirectory?: string, environment?: string, interpolate = false, skipRequiredVariables = false): Promise<RequestShowOutput> {
    const source = resolveSource(sourceDirectory);
    const result = await wasmCall('get_request_details', [await buildFilesMap(source), await buildSecretsMap(source), source, requestName, environment, interpolate, skipRequiredVariables]);
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
    const result = await wasmCall('get_auth_details', [await buildFilesMap(source), await buildSecretsMap(source), source, name, undefined, false]);
    const raw = JSON.parse(result) as AuthShowRaw;
    return { file: normalizePath(raw.file), line: raw.line, character: raw.character };
}

export async function showRequestLocation(requestName: string, sourceDirectory?: string): Promise<LocationOutput> {
    const source = resolveSource(sourceDirectory);
    const result = await wasmCall('get_request_details', [await buildFilesMap(source), await buildSecretsMap(source), source, requestName, undefined, false, false]);
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
    const filesJson = await buildFilesMap(workspace, drafts);

    for (const draft of drafts) {
        const raw = await wasmCall('lint', [filesJson, draft.source, draft.path]);
        results.set(draft.path, JSON.parse(raw) as LintResult);
    }
    return results;
}

export async function lintSource(source: string, filePath: string, workspaceDirectory?: string): Promise<LintResult> {
    const results = await lintSources([{ path: filePath, source }], workspaceDirectory);
    return results.get(draftKey(filePath)) ?? { ok: true, diagnostics: [] };
}

export async function checkFolder(folderPath: string, envName?: string): Promise<CheckResult> {
    const result = await wasmCall('check', [await buildFilesMap(folderPath), await buildSecretsMap(folderPath), folderPath, envName]);
    return JSON.parse(result) as CheckResult;
}

export async function executeRequest(options: ExecuteRequestOptions): Promise<ExecuteRequestResult> {
    const source = resolveSource(options.sourceDirectory);
    const [filesJson, secretsJson] = await Promise.all([buildFilesMap(source), buildSecretsMap(source)]);

    const variablesList = options.variables
        ? Object.entries(options.variables).map(([k, v]) => `${k}=${v}`)
        : undefined;
    const detailsRaw = await wasmCall('get_request_details', [
        filesJson, secretsJson, source,
        options.requestName, options.environment, true, false,
        variablesList ? JSON.stringify(variablesList) : undefined,
    ]);
    const raw = JSON.parse(detailsRaw) as RequestShowRaw;

    const url = raw.URL;
    const method = raw.Method;
    const body = raw.Body;
    const headers: Record<string, string> = { ...raw.Headers };
    const timeoutMs = (() => {
        if (!raw.Timeout) { return undefined; }
        const secs = Number(raw.Timeout);
        if (!Number.isFinite(secs) || secs < 0) {
            throw new Error(`Timeout value '${raw.Timeout}' must be a non-negative finite number`);
        }
        return secs * 1000;
    })();

    const existingHeaders = new Set(Object.keys(headers).map(k => k.toLowerCase()));

    if (!existingHeaders.has('user-agent')) {
        const version = await wasmCall('version', []);
        headers['user-agent'] = `rq/${version}`;
    }

    if (body && !existingHeaders.has('content-type') && isJsonBody(body)) {
        headers['content-type'] = 'application/json';
    }

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
        } else {
            throw new Error(`Auth configuration '${authName}' has unsupported type '${authType}'`);
        }
    }

    const startTime = Date.now();
    const response = await nodeHttpRequest(url, method, headers, body, timeoutMs);
    const elapsed = Date.now() - startTime;

    return {
        results: [{
            request_name: options.requestName,
            method,
            url,
            status: response.status,
            elapsed_ms: elapsed,
            request_headers: headers,
            request_body: body,
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

function nodeHttpRequest(url: string, method: string, reqHeaders: Record<string, string>, body?: string, timeoutMs?: number): Promise<NodeHttpResponse> {
    return new Promise((resolve, reject) => {
        const parsed = new URL(url);
        const isHttps = parsed.protocol === 'https:';
        const mod = isHttps ? https : http;

        const outHeaders: Record<string, string> = { ...reqHeaders };
        if (body) {
            outHeaders['content-length'] = String(Buffer.byteLength(body, 'utf8'));
        }

        let timer: ReturnType<typeof setTimeout> | undefined;
        const cleanup = () => { if (timer !== undefined) { clearTimeout(timer); timer = undefined; } };

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
            res.on('end', () => {
                cleanup();
                resolve({
                    status: res.statusCode ?? 0,
                    headers: responseHeaders,
                    body: Buffer.concat(chunks).toString('utf8'),
                });
            });
            res.on('error', (err) => { cleanup(); reject(err); });
        });

        req.on('error', (err) => { cleanup(); reject(err); });
        if (timeoutMs !== undefined) {
            timer = setTimeout(() => {
                req.destroy(new Error(`Request timed out after ${timeoutMs}ms`));
            }, timeoutMs);
        }
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

function isJsonBody(body: string): boolean {
    const t = body.trim();
    return (t.startsWith('{') && t.endsWith('}')) || (t.startsWith('[') && t.endsWith(']'));
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
    const p12Buffer = fs.readFileSync(p12Path);
    const p12Der = forge.util.binary.raw.encode(new Uint8Array(p12Buffer));
    const p12Asn1 = forge.asn1.fromDer(p12Der);
    let p12: forge.pkcs12.Pkcs12Pfx;
    try {
        p12 = forge.pkcs12.pkcs12FromAsn1(p12Asn1, password);
    } catch {
        if (password !== '') {
            throw new Error('Failed to parse .p12 certificate. Ensure the password is correct.');
        }
        try {
            p12 = forge.pkcs12.pkcs12FromAsn1(p12Asn1, false as unknown as string);
        } catch (e: unknown) {
            const msg = e instanceof Error ? e.message : String(e);
            throw new Error(`Failed to parse .p12 certificate. Ensure the password is correct. ${msg}`);
        }
    }

    const allCertBags = p12.getBags({ bagType: forge.pki.oids.certBag })[forge.pki.oids.certBag] ?? [];
    const allKeyBags = [
        ...(p12.getBags({ bagType: forge.pki.oids.pkcs8ShroudedKeyBag })[forge.pki.oids.pkcs8ShroudedKeyBag] ?? []),
        ...(p12.getBags({ bagType: forge.pki.oids.keyBag })[forge.pki.oids.keyBag] ?? []),
    ];

    if (allCertBags.length === 0) { throw new Error('No certificate found in P12'); }
    if (allKeyBags.length === 0)  { throw new Error('No private key found in P12'); }

    const localKeyId = (bag: forge.pkcs12.Bag) => bag.attributes?.localKeyId?.[0];

    let certBag = allCertBags[0];
    let keyBag  = allKeyBags[0];

    if (allCertBags.length > 1 || allKeyBags.length > 1) {
        for (const kb of allKeyBags) {
            const kid = localKeyId(kb);
            const matched = kid ? allCertBags.find(cb => localKeyId(cb) === kid) : undefined;
            if (matched) { certBag = matched; keyBag = kb; break; }
        }
    }

    if (!certBag.cert) { throw new Error('No certificate found in P12'); }
    if (!keyBag.key)   { throw new Error('No private key found in P12'); }

    const certDer = Buffer.from(forge.asn1.toDer(forge.pki.certificateToAsn1(certBag.cert)).getBytes(), 'binary');
    const privateKeyPem = Buffer.from(forge.pki.privateKeyToPem(keyBag.key));

    return { certDer, privateKeyPem };
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
