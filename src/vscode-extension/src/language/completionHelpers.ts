import * as vscode from 'vscode';

export const COMMON_HEADERS = [
    'Accept', 'Accept-Encoding', 'Accept-Language', 'Authorization',
    'Cache-Control', 'Content-Length', 'Content-Type', 'Cookie',
    'Host', 'If-Modified-Since', 'If-None-Match', 'Origin',
    'Referer', 'User-Agent', 'X-Api-Key', 'X-Auth-Token',
    'X-Correlation-Id', 'X-Forwarded-For', 'X-Request-Id',
];

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

export function builtinFunctionItems(): vscode.CompletionItem[] {
    return [
        (() => {
            const i = new vscode.CompletionItem('random.guid()', vscode.CompletionItemKind.Function);
            i.detail = 'random.guid() → string';
            i.insertText = 'random.guid()';
            return i;
        })(),
        (() => {
            const i = new vscode.CompletionItem('datetime.now()', vscode.CompletionItemKind.Function);
            i.detail = 'datetime.now(format?: string) → string';
            i.insertText = new vscode.SnippetString('datetime.now(${1:})');
            return i;
        })(),
        (() => {
            const i = new vscode.CompletionItem('io.read_file()', vscode.CompletionItemKind.Function);
            i.detail = 'io.read_file(path: string) → string';
            i.insertText = new vscode.SnippetString('io.read_file("${1:path}")');
            return i;
        })(),
    ];
}

export function dollarPrefixItems(inline = false): vscode.CompletionItem[] {
    const jsonItem = new vscode.CompletionItem('${ }', vscode.CompletionItemKind.Module);
    jsonItem.detail = 'JSON object literal';
    jsonItem.documentation = new vscode.MarkdownString('Inline JSON object literal.\n\n**Example:**\n```rq\nlet body = ${\n\t"key": "value"\n};\n```');
    jsonItem.insertText = inline
        ? new vscode.SnippetString('\\${${1:}}')
        : new vscode.SnippetString('\\${\n\t${1:}\n};');

    const headersItem = new vscode.CompletionItem('$[ ]', vscode.CompletionItemKind.Module);
    headersItem.detail = 'Headers dictionary';
    headersItem.documentation = new vscode.MarkdownString('Inline headers dictionary.\n\n**Example:**\n```rq\nlet h = $[];\n```');
    headersItem.insertText = inline
        ? new vscode.SnippetString('\\$[${1:}]')
        : new vscode.SnippetString('\\$[\n\t${1:}\n];');
    headersItem.command = { command: 'editor.action.triggerSuggest', title: 'Trigger header completions' };

    return [jsonItem, headersItem];
}

export function propertyItems(
    props: { name: string; signature: string; description: string; example: string }[],
    existingNamed: Set<string>,
    namedSuffix = false
): vscode.CompletionItem[] {
    return props
        .filter(p => !existingNamed.has(p.name))
        .map(prop => {
            const item = new vscode.CompletionItem(prop.name, vscode.CompletionItemKind.Property);
            item.detail = namedSuffix ? `${prop.signature} (named)` : prop.signature;
            item.documentation = new vscode.MarkdownString(
                `${prop.description}\n\n**Example:**\n\`\`\`rq\n${prop.example}\n\`\`\``
            );
            item.insertText = prop.name + ': ';
            item.command = { command: 'editor.action.triggerSuggest', title: 'Trigger value completions' };
            return item;
        });
}

export function insideOpenBlock(text: string, blockPattern: RegExp): boolean {
    let lastMatchEnd = -1;
    let m: RegExpExecArray | null;
    const re = new RegExp(blockPattern.source, blockPattern.flags.includes('g') ? blockPattern.flags : blockPattern.flags + 'g');
    while ((m = re.exec(text)) !== null) {
        lastMatchEnd = m.index + m[0].length;
    }
    if (lastMatchEnd === -1) { return false; }
    let depth = 1;
    for (const ch of text.slice(lastMatchEnd)) {
        if (ch === '{') { depth++; }
        else if (ch === '}') {
            depth--;
            if (depth === 0) { return false; }
        }
    }
    return depth > 0;
}

export const insideEnvOrAuthBlock = (text: string): boolean =>
    insideOpenBlock(text, /\b(env|auth)\s+\w+[^{]*\{/);

export const insideEpBody = (text: string): boolean =>
    insideOpenBlock(text, /\bep\s+\w+[^{;]*\{/);

export const insideArrayLiteral = (text: string): boolean => {
    let depth = 0;
    let inString = false;
    let stringChar = '';
    for (const ch of text) {
        if (inString) {
            if (ch === stringChar) { inString = false; }
        } else {
            if (ch === '"' || ch === "'") { inString = true; stringChar = ch; }
            else if (ch === '[') { depth++; }
            else if (ch === ']') { depth--; }
        }
    }
    return depth > 0;
};

export const insideJsonLiteral = (text: string): boolean => {
    const stack: ('json' | 'other')[] = [];
    let inString = false;
    let stringChar = '';
    for (let i = 0; i < text.length; i++) {
        const ch = text[i];
        if (inString) {
            if (ch === '\\') { i++; continue; }
            if (ch === stringChar) { inString = false; }
        } else {
            if (ch === '"' || ch === "'") { inString = true; stringChar = ch; }
            else if (ch === '$' && i + 1 < text.length && text[i + 1] === '{') {
                stack.push('json');
                i++;
            } else if (ch === '{') {
                stack.push('other');
            } else if (ch === '}') {
                stack.pop();
            }
        }
    }
    return stack.includes('json');
};

export const insideHeadersLiteral = (text: string): boolean => {
    const stack: ('headers' | 'array')[] = [];
    let inString = false;
    let stringChar = '';
    for (let i = 0; i < text.length; i++) {
        const ch = text[i];
        if (inString) {
            if (ch === stringChar) { inString = false; }
        } else {
            if (ch === '"' || ch === "'") { inString = true; stringChar = ch; }
            else if (ch === '$' && i + 1 < text.length && text[i + 1] === '[') {
                stack.push('headers');
                i++;
            } else if (ch === '[') {
                stack.push('array');
            } else if (ch === ']') {
                stack.pop();
            }
        }
    }
    return stack.length > 0 && stack[stack.length - 1] === 'headers';
};

export function getActiveAuthBlock(text: string): { authType: string; definedProps: Set<string> } | null {
    const headerRe = /\bauth\s+\w+\s*\(\s*auth_type\.(\w+)\s*\)\s*\{/g;
    let lastMatch: { authType: string; contentStart: number } | null = null;
    let m: RegExpExecArray | null;
    while ((m = headerRe.exec(text)) !== null) {
        lastMatch = { authType: m[1], contentStart: m.index + m[0].length };
    }
    if (!lastMatch) { return null; }
    const blockContent = text.slice(lastMatch.contentStart);
    let depth = 1;
    for (const ch of blockContent) {
        if (ch === '{') { depth++; }
        else if (ch === '}') {
            depth--;
            if (depth === 0) { return null; }
        }
    }
    const definedProps = new Set<string>();
    const propRe = /^\s*(\w+)\s*:/gm;
    let pm: RegExpExecArray | null;
    while ((pm = propRe.exec(blockContent)) !== null) {
        definedProps.add(pm[1]);
    }
    return { authType: lastMatch.authType, definedProps };
}

export function getCurrentEpBlockStartLine(documentPrefix: string): number {
    const re = /\bep\s+\w+[^{;]*\{/g;
    const epOpenerPositions: number[] = [];
    let m: RegExpExecArray | null;
    while ((m = re.exec(documentPrefix)) !== null) {
        epOpenerPositions.push(m.index + m[0].length - 1);
    }
    if (epOpenerPositions.length === 0) { return 0; }

    let depth = 0;
    let line = 0;
    let epIdx = 0;
    const epStack: Array<{ depth: number; line: number }> = [];
    for (let i = 0; i < documentPrefix.length; i++) {
        const ch = documentPrefix[i];
        if (ch === '\n') {
            line++;
        } else if (ch === '{') {
            depth++;
            if (epIdx < epOpenerPositions.length && epOpenerPositions[epIdx] === i) {
                epStack.push({ depth, line });
                epIdx++;
            }
        } else if (ch === '}') {
            if (epStack.length > 0 && epStack[epStack.length - 1].depth === depth) {
                epStack.pop();
            }
            depth--;
        }
    }
    return epStack.length > 0 ? epStack[epStack.length - 1].line : 0;
}

export function filterRequiredVars<T extends { source: string; line: number; file: string }>(
    variables: T[],
    documentPrefix: string,
    currentFile: string
): T[] {
    if (!insideEpBody(documentPrefix)) {
        return variables.filter(v => v.source !== 'required');
    }
    const epStartLine = getCurrentEpBlockStartLine(documentPrefix);
    return variables.filter(v =>
        v.source !== 'required' ||
        (v.file === currentFile && v.line >= epStartLine)
    );
}

export function collectNamedProps(text: string, propNames: string[]): Set<string> {
    const names = new Set<string>();
    const pattern = propNames.join('|');
    const re = new RegExp(`\\b(${pattern})\\s*:`, 'g');
    let m: RegExpExecArray | null;
    while ((m = re.exec(text)) !== null) {
        names.add(m[1]);
    }
    return names;
}

export function countPositionalArgs(matchedText: string): number {
    const parenIdx = matchedText.indexOf('(');
    if (parenIdx === -1) { return 0; }
    const argsText = matchedText.slice(parenIdx + 1);
    let depth = 0;
    let inString = false;
    let stringChar = '';
    let positional = 0;
    let segHasContent = false;
    let segIsNamed = false;
    for (const ch of argsText) {
        if (inString) {
            if (ch === stringChar) { inString = false; }
            segHasContent = true;
        } else if (ch === '"' || ch === "'") {
            inString = true; stringChar = ch; segHasContent = true;
        } else if (ch === '(' || ch === '[' || ch === '{') {
            depth++; segHasContent = true;
        } else if (ch === ')' || ch === ']' || ch === '}') {
            if (depth > 0) { depth--; }
        } else if (ch === ':' && depth === 0) {
            segIsNamed = true;
        } else if (ch === ',' && depth === 0) {
            if (!segIsNamed && segHasContent) { positional++; }
            segHasContent = false;
            segIsNamed = false;
        } else if (!/\s/.test(ch)) {
            segHasContent = true;
        }
    }
    return positional;
}
