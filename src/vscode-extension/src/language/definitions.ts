import * as vscode from 'vscode';

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
        parameters: ['format: string (optional) - The format string (e.g. "%Y-%m-%d")']
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
                value: match[2].trim(),
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

    const pattern = new RegExp(`\\[\\s*required\\s*\\(\\s*${escapeRegex(varName)}\\s*\\)`);
    for (let i = rqLine - 1; i >= 0; i--) {
        const text = document.lineAt(i).text;
        if (!/^\s*\[/.test(text)) {
            break;
        }
        if (pattern.test(text)) {
            return i;
        }
    }
    return -1;
}
