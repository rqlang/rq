import * as path from 'path';
import * as fs from 'fs';
import * as http from 'http';
import * as https from 'https';
import * as crypto from 'crypto';
import * as forge from 'node-forge';
import { buildSecretsMap, CANCELLED_MESSAGE, DraftFile, draftsJson } from './utils';
import { wasmCall } from './wasmHost';
import type { AuthShowOutput } from './rqClient';

export interface CancellationSignal {
    readonly isCancellationRequested: boolean;
    onCancellationRequested(listener: () => void): { dispose(): void };
}

export interface PreparedAuth extends AuthShowOutput {
    token_required: boolean;
}

export interface ExecuteRequestOptions {
    requestName: string;
    sourceDirectory?: string;
    sourceFile?: string;
    environment?: string;
    variables?: Record<string, string>;
    cancellation?: CancellationSignal;
    maxResponseBytes?: number;
    acquireToken?: (auth: PreparedAuth) => Promise<string>;
    confirm?: (prepared: PreparedRequest) => Promise<boolean>;
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
    body_truncated?: boolean;
}

export interface ExecuteRequestResult {
    results: RequestExecutionResult[];
    stderr?: string;
}

function toVariablesJson(variables?: Record<string, string>): string | undefined {
    if (!variables || Object.keys(variables).length === 0) {
        return undefined;
    }
    return JSON.stringify(Object.entries(variables).map(([k, v]) => `${k}=${v}`));
}

export function resolveScopeFile(scopeFile?: string): string | undefined {
    return scopeFile ? scopeFile.replace(/\\/g, '/') : undefined;
}

export interface PreparedRequest {
    name: string;
    method: string;
    url: string;
    headers: [string, string][];
    body?: string | null;
    timeout_ms?: number | null;
    auth?: PreparedAuth | null;
    confirmation_required: boolean;
    agent_confirmation_required: boolean;
    secret_values: string[];
}

export interface RequestScope {
    filesJson: string;
    secretsJson: string;
    source: string;
}

export async function requestScope(source: string, drafts: DraftFile[] = []): Promise<RequestScope> {
    return { filesJson: draftsJson(drafts), secretsJson: await buildSecretsMap(source), source };
}

export async function executeRequest(scope: RequestScope, options: ExecuteRequestOptions): Promise<ExecuteRequestResult> {
    throwIfCancelled(options.cancellation);
    const prepared = await prepareRequest(scope, options);
    throwIfCancelled(options.cancellation);
    if (prepared.confirmation_required && !(await options.confirm?.(prepared))) {
        throw new Error(CANCELLED_MESSAGE);
    }
    return sendRequest(scope, options, prepared);
}

export async function prepareRequest(scope: RequestScope, options: ExecuteRequestOptions, variables = options.variables): Promise<PreparedRequest> {
    return JSON.parse(
        await wasmCall('prepare_request', [scope.filesJson, scope.secretsJson, scope.source, options.requestName, options.environment, toVariablesJson(variables), resolveScopeFile(options.sourceFile)])
    ) as PreparedRequest;
}

export async function requestDefinition(scope: RequestScope, options: ExecuteRequestOptions): Promise<unknown> {
    return JSON.parse(
        await wasmCall('get_request_details', [scope.filesJson, scope.secretsJson, scope.source, options.requestName, options.environment, false, true, toVariablesJson(options.variables), resolveScopeFile(options.sourceFile)])
    );
}

export async function sendRequest(scope: RequestScope, options: ExecuteRequestOptions, preparedRequest: PreparedRequest): Promise<ExecuteRequestResult> {
    let prepared = preparedRequest;
    if (prepared.auth?.token_required) {
        const token = await acquireToken(prepared.auth, options);
        throwIfCancelled(options.cancellation);
        prepared = await prepareRequest(scope, options, { ...options.variables, auth_token: token });
    }

    const headers = toNodeHeaders(prepared.headers);
    const body = prepared.body ?? undefined;
    const startTime = Date.now();
    const response = await nodeHttpRequest(prepared.url, prepared.method, headers, body, prepared.timeout_ms ?? undefined, options.cancellation, options.maxResponseBytes);
    const elapsed = Date.now() - startTime;

    return {
        results: [{
            request_name: prepared.name,
            method: prepared.method,
            url: prepared.url,
            status: response.status,
            elapsed_ms: elapsed,
            request_headers: joinHeaderValues(headers),
            request_body: body,
            response_headers: response.headers,
            body: response.body,
            body_truncated: response.truncated,
        }],
    };
}

function toNodeHeaders(pairs: [string, string][]): Record<string, string | string[]> {
    const headers: Record<string, string | string[]> = {};
    const keysByLowerName = new Map<string, string>();
    for (const [name, value] of pairs) {
        const key = keysByLowerName.get(name.toLowerCase()) ?? name;
        keysByLowerName.set(name.toLowerCase(), key);
        const existing = headers[key];
        if (existing === undefined) {
            headers[key] = value;
        } else {
            headers[key] = Array.isArray(existing) ? [...existing, value] : [existing, value];
        }
    }
    return headers;
}

function joinHeaderValues(headers: Record<string, string | string[]>): Record<string, string> {
    return Object.fromEntries(
        Object.entries(headers).map(([name, value]) => [name, Array.isArray(value) ? value.join(', ') : value])
    );
}

async function acquireToken(auth: PreparedAuth, options: ExecuteRequestOptions): Promise<string> {
    if (auth.auth_type === 'oauth2_client_credentials') {
        return fetchClientCredentialsToken(auth.fields, path.dirname(auth.file));
    }
    if (options.acquireToken) {
        return options.acquireToken({ ...auth, environment: options.environment });
    }
    throw new Error(`Auth configuration '${auth.name}' (${auth.auth_type}) requires a token that could not be obtained`);
}

const MAX_TIMER_DELAY_MS = 2_147_483_647;

interface NodeHttpResponse {
    status: number;
    headers: Record<string, string>;
    body: string;
    truncated: boolean;
}

function throwIfCancelled(cancellation?: CancellationSignal): void {
    if (cancellation?.isCancellationRequested) {
        throw new Error(CANCELLED_MESSAGE);
    }
}

function nodeHttpRequest(url: string, method: string, reqHeaders: Record<string, string | string[]>, body?: string, timeoutMs?: number, cancellation?: CancellationSignal, maxBodyBytes = Infinity): Promise<NodeHttpResponse> {
    return new Promise((resolve, reject) => {
        if (cancellation?.isCancellationRequested) {
            reject(new Error(CANCELLED_MESSAGE));
            return;
        }
        const parsed = new URL(url);
        const isHttps = parsed.protocol === 'https:';
        const mod = isHttps ? https : http;

        const outHeaders: Record<string, string | string[]> = { ...reqHeaders };
        if (body) {
            outHeaders['content-length'] = String(Buffer.byteLength(body, 'utf8'));
        }

        let timer: ReturnType<typeof setTimeout> | undefined;
        let cancellationSubscription: { dispose(): void } | undefined;
        const cleanup = () => {
            if (timer !== undefined) { clearTimeout(timer); timer = undefined; }
            cancellationSubscription?.dispose();
            cancellationSubscription = undefined;
        };

        let req: http.ClientRequest | undefined;
        cancellationSubscription = cancellation?.onCancellationRequested(() => {
            req?.destroy(new Error(CANCELLED_MESSAGE));
        });

        req = mod.request({
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
            let received = 0;
            const finish = (truncated: boolean) => {
                cleanup();
                resolve({
                    status: res.statusCode ?? 0,
                    headers: responseHeaders,
                    body: Buffer.concat(chunks).subarray(0, maxBodyBytes).toString('utf8'),
                    truncated,
                });
            };
            res.on('data', (chunk: Buffer) => {
                chunks.push(chunk);
                received += chunk.length;
                if (received > maxBodyBytes) {
                    finish(true);
                    res.destroy();
                }
            });
            res.on('end', () => finish(false));
            res.on('error', (err) => { cleanup(); reject(err); });
        });

        req.on('error', (err) => { cleanup(); reject(err); });
        if (timeoutMs !== undefined && timeoutMs <= MAX_TIMER_DELAY_MS) {
            timer = setTimeout(() => {
                req?.destroy(new Error(`Request timed out after ${timeoutMs}ms`));
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

