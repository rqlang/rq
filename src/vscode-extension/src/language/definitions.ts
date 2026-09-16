import * as vscode from 'vscode';

export const IDENTIFIER_PATTERN = '[a-zA-Z_][a-zA-Z0-9_-]*';

export function declarationPattern(keyword: 'rq' | 'ep' | 'auth', captureName = false): string {
    const name = captureName ? `(${IDENTIFIER_PATTERN})` : IDENTIFIER_PATTERN;
    const template = keyword === 'ep' ? `(?:<\\s*${IDENTIFIER_PATTERN}\\s*>)?` : '';
    return `\\b${keyword}\\s+${name}\\s*${template}\\s*`;
}

export interface FunctionDefinition {
    name: string;
    signature: string;
    description: string;
    parameters: string[];
}

// System functions available in the rq language
export const SYSTEM_FUNCTIONS: FunctionDefinition[] = [
    // Add system functions here
];

// IO functions available in the rq language
export const IO_FUNCTIONS = [
    {
        name: 'read_file',
        signature: 'io.read_file(path: string)',
        description: 'Imports the contents of a file relative to the current .rq file',
        parameters: ['path: string - Relative or absolute path to the file to import']
    }
];

// Random functions available in the rq language
export const RANDOM_FUNCTIONS = [
    {
        name: 'guid',
        signature: 'random.guid()',
        description: 'Generates a random GUID (UUID v4)',
        parameters: []
    }
];

// DateTime functions available in the rq language
export const DATETIME_FUNCTIONS = [
    {
        name: 'now',
        signature: 'datetime.now(format?: string)',
        description: 'Returns the current date and time. If format is provided, it formats the date according to the format string. Otherwise it returns ISO 8601 format.',
        parameters: ['format: string (optional) - The format string (e.g. "yyyy-MM-dd HH:mm:ss")']
    }
];

// Request properties
export const REQUEST_PROPERTIES = [
    {
        name: 'url',
        signature: 'url: string',
        description: 'The URL for the HTTP request. Can include variable interpolation with {{variable}}',
        example: 'url: "https://api.example.com/users"'
    },
    {
        name: 'headers',
        signature: 'headers: $[string: string]',
        description: 'HTTP headers as key-value pairs',
        example: 'headers: $["Authorization": "Bearer {{token}}", "Content-Type": "application/json"]'
    },
    {
        name: 'body',
        signature: 'body: string | ${}',
        description: 'Request body content. Can be a string or JSON object (JSON must start with $)',
        example: 'body: ${"key": "value"} or body: sys.import_file("data.json") or body: "string"'
    }
];

// Endpoint properties
export const ENDPOINT_PROPERTIES = [
    {
        name: 'url',
        signature: 'url: string',
        description: 'Base URL for the endpoint. Child requests will inherit this URL',
        example: 'url: "https://api.example.com"'
    },
    {
        name: 'headers',
        signature: 'headers: $[string: string]',
        description: 'HTTP headers that will be inherited by all child requests',
        example: 'headers: $["Authorization": "Bearer {{token}}"]'
    },
    {
        name: 'qs',
        signature: 'qs: string',
        description: 'Query string that will be appended to all child request URLs',
        example: 'qs: "?version=1&format=json"'
    }
];

export const REQUEST_PARAM_NAMES = REQUEST_PROPERTIES.map(p => p.name);

export const ENDPOINT_PARAM_NAMES = ENDPOINT_PROPERTIES.map(p => p.name);

export const AUTH_PROPERTIES: Record<string, { name: string; required: boolean }[]> = {
    bearer: [
        { name: 'token', required: true },
    ],
    oauth2_client_credentials: [
        { name: 'client_id', required: true },
        { name: 'token_url', required: true },
        { name: 'client_secret', required: false },
        { name: 'cert_file', required: false },
        { name: 'cert_password', required: false },
        { name: 'scope', required: false },
    ],
    oauth2_authorization_code: [
        { name: 'client_id', required: true },
        { name: 'authorization_url', required: true },
        { name: 'token_url', required: true },
        { name: 'redirect_uri', required: false },
        { name: 'client_secret', required: false },
        { name: 'scope', required: false },
        { name: 'code_challenge_method', required: false },
        { name: 'use_state', required: false },
    ],
    oauth2_implicit: [
        { name: 'client_id', required: true },
        { name: 'authorization_url', required: true },
        { name: 'redirect_uri', required: false },
        { name: 'scope', required: false },
    ],
};

export interface AttributeDefinition {
    name: string;
    signature: string;
    description: string;
    targets: string;
    values?: string;
}

export const ATTRIBUTE_DEFINITIONS: AttributeDefinition[] = [
    {
        name: 'method',
        signature: '[method(VERB)]',
        description: 'Overrides the HTTP method regardless of the request name.',
        targets: 'rq',
        values: 'GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS'
    },
    {
        name: 'timeout',
        signature: '[timeout(seconds)]',
        description: 'Per-request timeout in seconds. On an ep it becomes the default for every child request.',
        targets: 'rq, ep'
    },
    {
        name: 'auth',
        signature: '[auth("provider_name")]',
        description: 'Attaches an auth provider. A name that resolves to an empty string disables auth for that request.',
        targets: 'rq, ep'
    },
    {
        name: 'required',
        signature: '[required(var_name)]',
        description: 'Declares a variable that must be supplied at runtime. May appear several times.',
        targets: 'rq'
    }
];

export function readDeclarationText(document: vscode.TextDocument, startLine: number, maxLines = 40): string {
    const lastLine = Math.min(document.lineCount - 1, startLine + maxLines);
    const lines: string[] = [];
    for (let i = startLine; i <= lastLine; i++) {
        lines.push(document.lineAt(i).text);
    }
    return lines.join('\n');
}

export function extractArgumentList(text: string, openIndex: number): string {
    let depth = 0;
    let inString = false;
    let stringChar = '';
    for (let i = openIndex; i < text.length; i++) {
        const ch = text[i];
        if (inString) {
            if (ch === '\\') { i++; }
            else if (ch === stringChar) { inString = false; }
            continue;
        }
        if (ch === '"' || ch === "'") { inString = true; stringChar = ch; }
        else if (ch === '(' || ch === '[' || ch === '{') { depth++; }
        else if (ch === ')' || ch === ']' || ch === '}') {
            depth--;
            if (depth === 0) { return text.slice(openIndex + 1, i); }
        }
    }
    return text.slice(openIndex + 1);
}

export function splitArguments(argsText: string): string[] {
    const segments: string[] = [];
    let current = '';
    let depth = 0;
    let inString = false;
    let stringChar = '';
    for (let i = 0; i < argsText.length; i++) {
        const ch = argsText[i];
        if (inString) {
            current += ch;
            if (ch === '\\' && i + 1 < argsText.length) { current += argsText[++i]; }
            else if (ch === stringChar) { inString = false; }
            continue;
        }
        if (ch === '"' || ch === "'") { inString = true; stringChar = ch; }
        else if (ch === '(' || ch === '[' || ch === '{') { depth++; }
        else if (ch === ')' || ch === ']' || ch === '}') { depth--; }
        else if (ch === ',' && depth === 0) {
            segments.push(current);
            current = '';
            continue;
        }
        current += ch;
    }
    segments.push(current);
    return segments.map(segment => segment.trim()).filter(segment => segment.length > 0);
}

export function assignArguments(argsText: string, paramNames: string[]): Map<string, string> {
    const assigned = new Map<string, string>();
    const namedPattern = new RegExp(`^(${paramNames.join('|')})\\s*:\\s*`);
    const positional: string[] = [];
    for (const segment of splitArguments(argsText)) {
        const named = namedPattern.exec(segment);
        if (named) {
            assigned.set(named[1], segment.slice(named[0].length).trim());
        } else {
            positional.push(segment);
        }
    }
    let index = 0;
    for (const value of positional) {
        while (index < paramNames.length && assigned.has(paramNames[index])) { index++; }
        if (index >= paramNames.length) { break; }
        assigned.set(paramNames[index], value);
        index++;
    }
    return assigned;
}

export interface Variable {
    name: string;
    value: string;
    line: number;
}

/**
 * Parse the document to extract all variable declarations
 * Matches patterns like: let variableName = "value" or let variableName = { ... }
 */
export function parseVariables(document: vscode.TextDocument): Variable[] {
    const variables: Variable[] = [];
    const varPattern = /^\s*let\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*=\s*(.+)$/;

    for (let i = 0; i < document.lineCount; i++) {
        const lineText = document.lineAt(i).text;
        const match = varPattern.exec(lineText);
        if (match) {
            variables.push({
                name: match[1],
                value: match[2].trim().replace(/;$/, ''),
                line: i
            });
        }
    }

    return variables;
}

function escapeRegex(s: string): string {
    return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

export function findRequiredAttributeLineInScope(
    document: vscode.TextDocument,
    cursorLine: number,
    varName: string
): number {
    let rqLine = -1;
    for (let i = cursorLine; i >= 0; i--) {
        if (/^\s*rq\s+/.test(document.lineAt(i).text)) {
            rqLine = i;
            break;
        }
    }
    if (rqLine === -1) {
        return -1;
    }

    for (let i = rqLine - 1; i >= 0; i--) {
        const text = document.lineAt(i).text;
        if (!/^\s*\[/.test(text)) {
            break;
        }
        if (isRequiredAttributeFor(text, varName)) {
            return i;
        }
    }
    return -1;
}

export function isRequiredAttributeFor(lineText: string, varName: string): boolean {
    return new RegExp(`\\[\\s*required\\s*\\(\\s*${escapeRegex(varName)}\\s*\\)`).test(lineText);
}
