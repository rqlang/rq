import * as vscode from 'vscode';
import {
    REQUEST_PARAM_NAMES,
    ENDPOINT_PARAM_NAMES,
    AUTH_PROPERTIES,
    AttributeDefinition,
    readDeclarationText,
    extractArgumentList,
    assignArguments,
    splitArguments,
} from './definitions';

const NOT_SET = '<not set>';
const MAX_VALUE_LENGTH = 60;
const MAX_ENV_ENTRIES = 12;
const HTTP_METHODS = ['get', 'post', 'put', 'delete', 'patch', 'head', 'options'];

export interface ArtifactAttributes {
    method?: string;
    timeout?: string;
    auth?: string;
    required: string[];
}

function formatAuthType(authType: string): string {
    const types: Record<string, string> = {
        bearer: 'Bearer Token',
        oauth2_client_credentials: 'OAuth2 Client Credentials',
        oauth2_authorization_code: 'OAuth2 Authorization Code (PKCE)',
        oauth2_implicit: 'OAuth2 Implicit Flow',
    };
    return types[authType] ?? authType;
}

function formatValue(raw: string): string {
    const collapsed = raw.replace(/\s+/g, ' ').trim();
    if (collapsed.length === 0) { return NOT_SET; }
    return collapsed.length > MAX_VALUE_LENGTH
        ? `${collapsed.slice(0, MAX_VALUE_LENGTH - 1)}…`
        : collapsed;
}

function buildBlock(header: string, entries: [string, string][], closer: string): string {
    if (entries.length === 0) { return `${header}${closer}`; }
    const width = Math.max(...entries.map(([name]) => name.length));
    const lines = entries.map(([name, value]) => `  ${name}:${' '.repeat(width - name.length + 1)}${value},`);
    return [header, ...lines, closer].join('\n');
}

function parameterEntries(
    document: vscode.TextDocument,
    line: number,
    openIndex: number,
    paramNames: string[]
): [string, string][] {
    const declaration = readDeclarationText(document, line);
    const argsText = extractArgumentList(declaration, openIndex);
    const assigned = assignArguments(argsText, paramNames);
    return paramNames.map(name => {
        const value = assigned.get(name);
        return [name, value === undefined ? NOT_SET : formatValue(value)] as [string, string];
    });
}

export function collectAttributes(document: vscode.TextDocument, declarationLine: number): ArtifactAttributes {
    const attributes: ArtifactAttributes = { required: [] };
    for (let i = declarationLine - 1; i >= 0; i--) {
        const line = document.lineAt(i).text.trim();
        if (line.length === 0) { continue; }
        if (!line.startsWith('[')) { break; }
        const method = /\[method\(\s*([^)]*?)\s*\)\]/.exec(line);
        if (method) { attributes.method = method[1].toUpperCase(); }
        const timeout = /\[timeout\(\s*([^)]*?)\s*\)\]/.exec(line);
        if (timeout) { attributes.timeout = timeout[1]; }
        const auth = /\[auth\(\s*"?([^")]*?)"?\s*\)\]/.exec(line);
        if (auth) { attributes.auth = auth[1]; }
        const required = /\[required\(\s*([^)]*?)\s*\)\]/.exec(line);
        if (required) { attributes.required.unshift(required[1]); }
    }
    return attributes;
}

function appendAttributes(contents: vscode.MarkdownString, attributes: ArtifactAttributes, method?: string) {
    const parts: string[] = [];
    if (method) { parts.push(`**Method:** \`${method}\``); }
    if (attributes.timeout) { parts.push(`**Timeout:** \`${attributes.timeout}s\``); }
    if (attributes.auth) { parts.push(`**Auth:** \`${attributes.auth}\``); }
    if (parts.length > 0) { contents.appendMarkdown(`\n${parts.join(' · ')}`); }
    if (attributes.required.length > 0) {
        contents.appendMarkdown(`\n\n**Required:** ${attributes.required.map(v => `\`${v}\``).join(', ')}`);
    }
}

export function buildRequestHover(
    document: vscode.TextDocument,
    line: number,
    name: string,
    openIndex: number
): vscode.MarkdownString {
    const entries = parameterEntries(document, line, openIndex, REQUEST_PARAM_NAMES);
    const attributes = collectAttributes(document, line);
    const method = attributes.method
        ?? (HTTP_METHODS.includes(name.toLowerCase()) ? name.toUpperCase() : 'GET');
    const contents = new vscode.MarkdownString();
    contents.appendMarkdown(`**Request: \`${name}\`**\n\n`);
    contents.appendCodeblock(buildBlock(`rq ${name}(`, entries, ')'), 'rq');
    appendAttributes(contents, attributes, method);
    return contents;
}

export function buildEndpointHover(
    document: vscode.TextDocument,
    line: number,
    name: string,
    parent: string | undefined,
    openIndex: number
): vscode.MarkdownString {
    const isBlockOnly = document.lineAt(line).text[openIndex] === '{';
    const entries = isBlockOnly
        ? ENDPOINT_PARAM_NAMES.map(param => [param, NOT_SET] as [string, string])
        : parameterEntries(document, line, openIndex, ENDPOINT_PARAM_NAMES);
    const attributes = collectAttributes(document, line);
    const header = `ep ${name}${parent ? `<${parent}>` : ''}(`;
    const contents = new vscode.MarkdownString();
    contents.appendMarkdown(`**Endpoint: \`${name}\`**\n\n`);
    contents.appendCodeblock(buildBlock(header, entries, ')'), 'rq');
    if (parent) { contents.appendMarkdown(`\n**Extends:** \`${parent}\``); }
    appendAttributes(contents, attributes);
    return contents;
}

export function buildAuthHover(
    document: vscode.TextDocument,
    line: number,
    name: string,
    authType: string
): vscode.MarkdownString {
    const fields = AUTH_PROPERTIES[authType];
    const contents = new vscode.MarkdownString();
    contents.appendMarkdown(`**Auth: \`${name}\`** *(${formatAuthType(authType)})*\n\n`);
    if (!fields) {
        contents.appendCodeblock(`auth ${name}(auth_type.${authType})`, 'rq');
        return contents;
    }
    const defined = parseBlockEntries(document, line);
    const entries = fields.map(field => {
        const value = defined.get(field.name);
        return [field.name, value === undefined ? NOT_SET : formatValue(value)] as [string, string];
    });
    const header = `auth ${name}(auth_type.${authType}) {`;
    contents.appendCodeblock(buildBlock(header, entries, '}'), 'rq');
    const required = fields.filter(field => field.required);
    contents.appendMarkdown(`\n**Required fields:** ${required.map(f => `\`${f.name}\``).join(', ')}`);
    const missing = required.filter(field => !defined.has(field.name));
    if (missing.length > 0) {
        contents.appendMarkdown(`\n\n**Missing:** ${missing.map(f => `\`${f.name}\``).join(', ')}`);
    }
    return contents;
}

export function parseBlockEntries(document: vscode.TextDocument, startLine: number): Map<string, string> {
    const entries = new Map<string, string>();
    const text = readDeclarationText(document, startLine, 80);
    const braceIndex = text.indexOf('{');
    if (braceIndex === -1) { return entries; }
    const body = extractArgumentList(text, braceIndex);
    for (const segment of splitArguments(body)) {
        const match = /^"?([a-zA-Z_][a-zA-Z0-9_-]*)"?\s*:\s*/.exec(segment);
        if (match) {
            entries.set(match[1], segment.slice(match[0].length).trim());
        }
    }
    return entries;
}

export function buildEnvironmentHover(
    document: vscode.TextDocument,
    line: number,
    name: string
): vscode.MarkdownString {
    const defined = [...parseBlockEntries(document, line)];
    const shown = defined.slice(0, MAX_ENV_ENTRIES);
    const entries = shown.map(([key, value]) => [key, formatValue(value)] as [string, string]);
    const contents = new vscode.MarkdownString();
    contents.appendMarkdown(`**Environment: \`${name}\`**\n\n`);
    contents.appendCodeblock(buildBlock(`env ${name} {`, entries, '}'), 'rq');
    if (defined.length === 0) {
        contents.appendMarkdown('\nNo variables defined.');
    } else if (defined.length > shown.length) {
        contents.appendMarkdown(`\n*(+${defined.length - shown.length} more)*`);
    }
    return contents;
}

export function buildImportHover(target: string): vscode.MarkdownString {
    const file = target.endsWith('.rq') ? target : `${target}.rq`;
    const contents = new vscode.MarkdownString();
    contents.appendMarkdown(`**Import: \`${file}\`**\n\n`);
    contents.appendCodeblock(`import "${target}";`, 'rq');
    contents.appendMarkdown('\nMerges every request, variable, environment, auth provider and endpoint from that file.');
    contents.appendMarkdown('\n\nPath is resolved relative to the current file; the extension is optional. Circular imports are not allowed.');
    return contents;
}

export function buildAttributeHover(definition: AttributeDefinition): vscode.MarkdownString {
    const contents = new vscode.MarkdownString();
    contents.appendMarkdown(`**Attribute: \`${definition.name}\`**\n\n`);
    contents.appendCodeblock(definition.signature, 'rq');
    contents.appendMarkdown(`\n${definition.description}`);
    contents.appendMarkdown(`\n\n**Applies to:** \`${definition.targets}\``);
    if (definition.values) {
        contents.appendMarkdown(`\n\n**Values:** ${definition.values}`);
    }
    return contents;
}
