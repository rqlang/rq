import * as vscode from 'vscode';
import * as path from 'path';
import * as cliService from '../cliService';
import {
    SYSTEM_FUNCTIONS,
    IO_FUNCTIONS,
    RANDOM_FUNCTIONS,
    DATETIME_FUNCTIONS,
    REQUEST_PROPERTIES,
    ENDPOINT_PROPERTIES,
    parseVariables
} from './definitions';

let environmentProvider: { getSelectedEnvironment(): string | undefined } | undefined;

export function setEnvironmentProvider(provider: { getSelectedEnvironment(): string | undefined }) {
    environmentProvider = provider;
}

const COMMON_HEADERS = [
    'Accept', 'Accept-Encoding', 'Accept-Language', 'Authorization',
    'Cache-Control', 'Content-Length', 'Content-Type', 'Cookie',
    'Host', 'If-Modified-Since', 'If-None-Match', 'Origin',
    'Referer', 'User-Agent', 'X-Api-Key', 'X-Auth-Token',
    'X-Correlation-Id', 'X-Forwarded-For', 'X-Request-Id',
];

// Helper: collect named properties already present in an rq(...) block text
const collectRqNamed = (text: string): Set<string> => {
    const names = new Set<string>();
    const re = /\b(url|headers|body|method)\s*:/g;
    let m: RegExpExecArray | null;
    while ((m = re.exec(text)) !== null) {
        names.add(m[1]);
    }
    return names;
};

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

const AUTH_PROPERTIES: Record<string, { name: string; required: boolean }[]> = {
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
        { name: 'scope', required: false },
        { name: 'code_challenge_method', required: false },
    ],
    oauth2_implicit: [
        { name: 'client_id', required: true },
        { name: 'authorization_url', required: true },
        { name: 'scope', required: false },
    ],
};

const getActiveAuthBlock = (text: string): { authType: string; definedProps: Set<string> } | null => {
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
};

const insideEnvOrAuthBlock = (text: string): boolean => {
    const re = /\b(env|auth)\s+\w+[^{]*\{/g;
    let lastMatchEnd = -1;
    let m: RegExpExecArray | null;
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
};

// Helper: collect named properties already present in an ep(...) block text
const collectEpNamed = (text: string): Set<string> => {
    const names = new Set<string>();
    const re = /\b(url|headers|qs)\s*:/g;
    let m: RegExpExecArray | null;
    while ((m = re.exec(text)) !== null) {
        names.add(m[1]);
    }
    return names;
};

const countPositionalArgs = (matchedText: string): number => {
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
};

const builtinFunctionItems = (): vscode.CompletionItem[] => [
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

export const completionProvider = vscode.languages.registerCompletionItemProvider(
    'rq',
    {
        async provideCompletionItems(document: vscode.TextDocument, position: vscode.Position) {
            const linePrefix = document.lineAt(position).text.substring(0, position.character);

            // Endpoint template completion: ep name< -> list existing endpoints
            const epTemplateMatch = linePrefix.match(/^\s*ep\s+[a-zA-Z_][a-zA-Z0-9_-]*\s*<$/);
            if (epTemplateMatch) {
                const sourceDirectory = document.uri.fsPath;
                try {
                    const endpoints = await cliService.listEndpoints(sourceDirectory);
                    return endpoints.filter(ep => ep.is_template).map(ep => {
                        const item = new vscode.CompletionItem(ep.name, vscode.CompletionItemKind.Reference);
                        item.detail = 'Endpoint template';
                        item.insertText = ep.name;
                        return item;
                    });
                } catch {
                    return undefined;
                }
            }

            // Import directive completion without duplicating the keyword
            // Cases:
            //   1) User typed 'import' (no trailing space) -> suggest full snippet including keyword.
            //   2) User typed 'import ' (has trailing space) -> only insert the path + quotes/semicolon.
            const importFullMatch = /^\s*import$/; // exact 'import'
            const importSpaceMatch = /^\s*import\s+$/; // 'import ' with trailing space(s)
            if (importFullMatch.test(linePrefix) || importSpaceMatch.test(linePrefix)) {
                const prevText = document.getText(new vscode.Range(new vscode.Position(0, 0), position));
                const inRqOrEp = /\b(rq|ep)\s+\w+\s*\(/.test(prevText);
                if (!inRqOrEp) {
                    const hasTrailingSpace = importSpaceMatch.test(linePrefix);
                    const currentDir = path.dirname(document.uri.fsPath);
                    const allFiles = await vscode.workspace.findFiles('**/*.rq');
                    const otherFiles = allFiles.filter(u => u.fsPath !== document.uri.fsPath);

                    return otherFiles.map(fileUri => {
                        const relativePath = path.relative(currentDir, fileUri.fsPath).replace(/\.rq$/, '').replace(/\\/g, '/');
                        const item = new vscode.CompletionItem(relativePath, vscode.CompletionItemKind.File);
                        item.detail = 'Import .rq file';
                        item.insertText = hasTrailingSpace
                            ? `"${relativePath}";`
                            : ` "${relativePath}";`;
                        item.commitCharacters = [';'];
                        return item;
                    });
                }
            }

            // Variable reference completion: let name = <cursor> -> suggest existing variables + functions
            if (/^\s*let\s+[a-zA-Z_][a-zA-Z0-9_-]*\s*=\s*$/.test(linePrefix)) {
                const suggestions: vscode.CompletionItem[] = [...builtinFunctionItems()];
                try {
                    const variables = await cliService.listVariables(document.uri.fsPath, environmentProvider?.getSelectedEnvironment());
                    variables.forEach(v => {
                        const item = new vscode.CompletionItem(v.name, vscode.CompletionItemKind.Variable);
                        item.detail = v.value ? `= ${v.value}` : v.source;
                        item.insertText = `${v.name};`;
                        suggestions.push(item);
                    });
                } catch { /* ignore */ }
                return suggestions;
            }

            // Variable interpolation completion: inside {{ (with optional partial name already typed)
            const interpolationMatch = linePrefix.match(/\{\{([a-zA-Z0-9_-]*)$/);
            if (interpolationMatch) {
                const partial = interpolationMatch[1];
                let replaceRange: vscode.Range | undefined;
                if (partial.length > 0) {
                    const afterCursor = document.lineAt(position.line).text.substring(position.character);
                    const trailingName = afterCursor.match(/^([a-zA-Z0-9_-]*)/)?.[1] ?? '';
                    replaceRange = new vscode.Range(
                        position.line, position.character - partial.length,
                        position.line, position.character + trailingName.length
                    );
                }
                try {
                    const variables = await cliService.listVariables(document.uri.fsPath, environmentProvider?.getSelectedEnvironment());
                    if (variables.length > 0) {
                        return variables.map(v => {
                            const item = new vscode.CompletionItem(v.name, vscode.CompletionItemKind.Variable);
                            item.detail = v.value ? `= ${v.value}` : v.source;
                            item.insertText = v.name;
                            if (replaceRange) { item.range = replaceRange; }
                            return item;
                        });
                    }
                } catch { /* fall through to local variables */ }
                const localVars = parseVariables(document);
                return localVars.length > 0 ? localVars.map(v => {
                    const item = new vscode.CompletionItem(v.name, vscode.CompletionItemKind.Variable);
                    item.detail = `Variable (line ${v.line + 1})`;
                    item.documentation = new vscode.MarkdownString(`Value: \`${v.value}\``);
                    item.insertText = v.name;
                    if (replaceRange) { item.range = replaceRange; }
                    return item;
                }) : undefined;
            }

            // Property value completion inside env/auth blocks
            // Triggers when cursor is at the start of a value: `prop: ` or `prop: "`
            if (/^\s*"?[a-zA-Z_][a-zA-Z0-9_-]*"?\s*:\s*"?$/.test(linePrefix)) {
                const blockText = document.getText(new vscode.Range(
                    new vscode.Position(Math.max(0, position.line - 30), 0),
                    position
                ));
                if (insideEnvOrAuthBlock(blockText) || insideArrayLiteral(blockText)) {
                    const suggestions: vscode.CompletionItem[] = [...builtinFunctionItems()];
                    let gotRemoteVars = false;
                    try {
                        const variables = await cliService.listVariables(document.uri.fsPath, environmentProvider?.getSelectedEnvironment());
                        if (variables.length > 0) {
                            gotRemoteVars = true;
                            variables.forEach(v => {
                                const item = new vscode.CompletionItem(v.name, vscode.CompletionItemKind.Variable);
                                item.detail = v.value ? `= ${v.value}` : v.source;
                                item.insertText = v.name;
                                suggestions.push(item);
                            });
                        }
                    } catch { /* fall through to local variables */ }
                    if (!gotRemoteVars) {
                        parseVariables(document).forEach(v => {
                            const item = new vscode.CompletionItem(v.name, vscode.CompletionItemKind.Variable);
                            item.detail = `Variable (line ${v.line + 1})`;
                            item.documentation = new vscode.MarkdownString(`Value: \`${v.value}\``);
                            item.insertText = v.name;
                            suggestions.push(item);
                        });
                    }
                    return suggestions;
                }
            }

            // Auth property name completion inside auth blocks
            if (/^\s*\w*$/.test(linePrefix)) {
                const blockText = document.getText(new vscode.Range(
                    new vscode.Position(Math.max(0, position.line - 50), 0),
                    position
                ));
                const authBlock = getActiveAuthBlock(blockText);
                if (authBlock) {
                    const props = AUTH_PROPERTIES[authBlock.authType];
                    if (props) {
                        return props
                            .filter(p => !authBlock.definedProps.has(p.name))
                            .map(p => {
                                const item = new vscode.CompletionItem(p.name, vscode.CompletionItemKind.Property);
                                item.detail = p.required ? 'required' : 'optional';
                                item.insertText = new vscode.SnippetString(`${p.name}: "\${1:}"`);
                                item.sortText = p.required ? `0${p.name}` : `1${p.name}`;
                                return item;
                            });
                    }
                }
            }

            // Variable declaration templates
            // Cases:
            // 1) 'let' alone -> offer all variants (insert leading space before name).
            // 2) 'let ' with trailing space -> offer variants WITHOUT repeating the keyword.
            // 3) 'let <partialName>' -> still offer variants, replacing the partial name.
            // Avoid inside rq(...) or ep(...)
            const letExact = /^\s*let$/.test(linePrefix);
            const letWithSpace = /^\s*let\s+$/.test(linePrefix);
            const letWithName = /^\s*let\s+[a-zA-Z_][a-zA-Z0-9_-]*$/.test(linePrefix);
            if (/^\s*let(\s+[a-zA-Z_]?[a-zA-Z0-9_-]*)?$/.test(linePrefix)) {
                const prevText = document.getText(new vscode.Range(new vscode.Position(0, 0), position));
                const inRqOrEp = /\b(rq|ep)\s+\w+\s*\(/.test(prevText);
                if (!inRqOrEp) {
                    const variants: vscode.CompletionItem[] = [];

                    // Determine snippet prefix strategy
                    // We do NOT want to duplicate 'let' if already typed followed by a space.
                    // If only 'let' (no space) is typed, we prepend a space before the variable name.
                    // If 'let <partial>' typed, we replace the partial with placeholder by using a range edit.
                    let prefixForNewDecl = 'let ';
                    if (letWithSpace) {
                        prefixForNewDecl = ''; // user already has 'let '
                    } else if (letExact) {
                        prefixForNewDecl = ' '; // user typed 'let' without trailing space
                    } else if (letWithName) {
                        prefixForNewDecl = ''; // we'll replace existing name via range edits
                    }

                    // Compute replacement range when a partial name exists so we don't duplicate text
                    let replacementRange: vscode.Range | undefined;
                    if (letWithName) {
                        const lineText = document.lineAt(position.line).text;
                        const nameMatch = lineText.match(/^\s*let\s+([a-zA-Z_][a-zA-Z0-9_-]*)$/);
                        if (nameMatch) {
                            const nameStart = lineText.indexOf(nameMatch[1]);
                            const nameEnd = nameStart + nameMatch[1].length;
                            replacementRange = new vscode.Range(position.line, nameStart, position.line, nameEnd);
                        }
                    }

                    // 1. String variable
                    const strItem = new vscode.CompletionItem('let string', vscode.CompletionItemKind.Keyword);
                    strItem.detail = 'String variable';
                    strItem.documentation = new vscode.MarkdownString('Declare a string variable');
                    strItem.insertText = new vscode.SnippetString(`${prefixForNewDecl}\${1:name} = "\${2:value}"`);
                    if (replacementRange) { strItem.range = replacementRange; }
                    variants.push(strItem);

                    // 2. JSON object variable
                    const jsonItem = new vscode.CompletionItem('let json', vscode.CompletionItemKind.Keyword);
                    jsonItem.detail = 'JSON object variable';
                    jsonItem.documentation = new vscode.MarkdownString('Declare a JSON object variable');
                    jsonItem.insertText = new vscode.SnippetString(`${prefixForNewDecl}\${1:name} = {\n    "\${2:key}": "\${3:value}"\n}`);
                    if (replacementRange) { jsonItem.range = replacementRange; }
                    variants.push(jsonItem);

                    // 3. Request URL variable
                    const urlItem = new vscode.CompletionItem('let url', vscode.CompletionItemKind.Keyword);
                    urlItem.detail = 'URL variable';
                    urlItem.documentation = new vscode.MarkdownString('Declare a URL variable');
                    urlItem.insertText = new vscode.SnippetString(`${prefixForNewDecl}\${1:base_url} = "https://api.example.com"`);
                    if (replacementRange) { urlItem.range = replacementRange; }
                    variants.push(urlItem);

                    return variants;
                }
            }

            // Check if we're typing "auth_type." inside an auth block
            if (linePrefix.endsWith('auth_type.') || /auth_type\.\w*$/.test(linePrefix)) {
                const authTypes = [
                    {
                        name: 'bearer',
                        detail: 'Bearer Token Authentication',
                        description: 'Simple bearer token authentication. Requires: token',
                        insertText: 'bearer'
                    },
                    {
                        name: 'oauth2_authorization_code',
                        detail: 'OAuth2 Authorization Code with PKCE',
                        description: 'OAuth2 authorization code flow with PKCE. Requires: client_id, authorization_url, token_url, redirect_uri, scope, code_challenge_method',
                        insertText: 'oauth2_authorization_code'
                    },
                    {
                        name: 'oauth2_client_credentials',
                        detail: 'OAuth2 Client Credentials',
                        description: 'OAuth2 client credentials flow. Requires: client_id, token_url. Authentication via client_secret or cert_file (private_key_jwt)',
                        insertText: 'oauth2_client_credentials'
                    },
                    {
                        name: 'oauth2_implicit',
                        detail: 'OAuth2 Implicit Flow',
                        description: 'OAuth2 implicit flow. Requires: client_id, authorization_url, redirect_uri, scope',
                        insertText: 'oauth2_implicit'
                    }
                ];

                return authTypes.map(authType => {
                    const item = new vscode.CompletionItem(authType.name, vscode.CompletionItemKind.EnumMember);
                    item.detail = authType.detail;
                    item.documentation = new vscode.MarkdownString(authType.description);
                    item.insertText = authType.insertText;
                    return item;
                });
            }
            
            // Check if we're inside an rq block (after comma or opening parenthesis)
            // Look for pattern: rq name(..., or rq name(
            const textBeforeCursor = document.getText(new vscode.Range(
                new vscode.Position(Math.max(0, position.line - 10), 0),
                position
            ));
            
            // Check if we're inside an rq declaration
            const rqMatch = textBeforeCursor.match(/\brq\s+\w+\s*\([^;]*$/s);
            if (rqMatch) {
                const matchedText = rqMatch[0];
                const hasNamedParams = /\b(url|headers|body|method)\s*:/.test(matchedText);
                const atStartOfParams = /\brq\s+\w+\s*\(\s*$/.test(linePrefix);
                const afterComma = /,\s*$/.test(linePrefix);
                const onNewLine = /^\s*$/.test(linePrefix) && !!rqMatch;
                const atNamedValue = /\b(url|headers|body|method)\s*:\s*$/.test(linePrefix);

                if (atStartOfParams || afterComma || onNewLine || atNamedValue) {
                    if (atNamedValue) {
                        const suggestions: vscode.CompletionItem[] = [...builtinFunctionItems()];
                        let gotRemoteVars = false;
                        try {
                            const variables = await cliService.listVariables(document.uri.fsPath, environmentProvider?.getSelectedEnvironment());
                            if (variables.length > 0) {
                                gotRemoteVars = true;
                                variables.forEach(v => {
                                    const item = new vscode.CompletionItem(v.name, vscode.CompletionItemKind.Variable);
                                    item.detail = v.value ? `= ${v.value}` : v.source;
                                    item.insertText = v.name;
                                    suggestions.push(item);
                                });
                            }
                        } catch { /* ignore */ }
                        if (!gotRemoteVars) {
                            parseVariables(document).forEach(v => {
                                const item = new vscode.CompletionItem(v.name, vscode.CompletionItemKind.Variable);
                                item.detail = `Variable (line ${v.line + 1})`;
                                item.insertText = v.name;
                                suggestions.push(item);
                            });
                        }
                        return suggestions;
                    }

                    const existingNamed = collectRqNamed(matchedText);
                    const positionalCount = countPositionalArgs(matchedText);
                    ['url', 'headers', 'body'].slice(0, positionalCount).forEach(p => existingNamed.add(p));

                    if (hasNamedParams) {
                        return REQUEST_PROPERTIES
                            .filter(p => !existingNamed.has(p.name))
                            .map(prop => {
                                const item = new vscode.CompletionItem(prop.name, vscode.CompletionItemKind.Property);
                                item.detail = prop.signature;
                                item.documentation = new vscode.MarkdownString(
                                    `${prop.description}\n\n**Example:**\n\`\`\`rq\n${prop.example}\n\`\`\``
                                );
                                item.insertText = prop.name + ': ';
                                return item;
                            });
                    }

                    const suggestions: vscode.CompletionItem[] = [...builtinFunctionItems()];
                    try {
                        const variables = await cliService.listVariables(document.uri.fsPath, environmentProvider?.getSelectedEnvironment());
                        variables.forEach(v => {
                            const item = new vscode.CompletionItem(v.name, vscode.CompletionItemKind.Variable);
                            item.detail = v.value ? `= ${v.value}` : v.source;
                            item.insertText = v.name;
                            suggestions.push(item);
                        });
                    } catch { /* ignore */ }
                    REQUEST_PROPERTIES.forEach(prop => {
                        if (existingNamed.has(prop.name)) { return; }
                        const item = new vscode.CompletionItem(prop.name, vscode.CompletionItemKind.Property);
                        item.detail = prop.signature + ' (named)';
                        item.documentation = new vscode.MarkdownString(
                            `${prop.description}\n\n**Example:**\n\`\`\`rq\n${prop.example}\n\`\`\``
                        );
                        item.insertText = prop.name + ': ';
                        suggestions.push(item);
                    });
                    return suggestions;
                }
            }
            
            // Check if we're inside an ep (endpoint) declaration
            const epMatch = textBeforeCursor.match(/\bep\s+\w+\s*\([^{;]*$/s);
            if (epMatch) {
                const matchedText = epMatch[0];
                const hasNamedParams = /\b(url|headers|qs)\s*:/.test(matchedText);
                const atStartOfParams = /\bep\s+\w+\s*\(\s*$/.test(linePrefix);
                const afterComma = /,\s*$/.test(linePrefix);
                const onNewLine = /^\s*$/.test(linePrefix) && !!epMatch;
                const atNamedValue = /\b(url|headers|qs)\s*:\s*$/.test(linePrefix);

                if (atStartOfParams || afterComma || onNewLine || atNamedValue) {
                    if (atNamedValue) {
                        const suggestions: vscode.CompletionItem[] = [...builtinFunctionItems()];
                        let gotRemoteVars = false;
                        try {
                            const variables = await cliService.listVariables(document.uri.fsPath, environmentProvider?.getSelectedEnvironment());
                            if (variables.length > 0) {
                                gotRemoteVars = true;
                                variables.forEach(v => {
                                    const item = new vscode.CompletionItem(v.name, vscode.CompletionItemKind.Variable);
                                    item.detail = v.value ? `= ${v.value}` : v.source;
                                    item.insertText = v.name;
                                    suggestions.push(item);
                                });
                            }
                        } catch { /* ignore */ }
                        if (!gotRemoteVars) {
                            parseVariables(document).forEach(v => {
                                const item = new vscode.CompletionItem(v.name, vscode.CompletionItemKind.Variable);
                                item.detail = `Variable (line ${v.line + 1})`;
                                item.insertText = v.name;
                                suggestions.push(item);
                            });
                        }
                        return suggestions;
                    }

                    const existingNamedEp = collectEpNamed(matchedText);
                    const positionalCount = countPositionalArgs(matchedText);
                    ['url', 'headers', 'qs'].slice(0, positionalCount).forEach(p => existingNamedEp.add(p));

                    if (hasNamedParams) {
                        return ENDPOINT_PROPERTIES
                            .filter(p => !existingNamedEp.has(p.name))
                            .map(prop => {
                                const item = new vscode.CompletionItem(prop.name, vscode.CompletionItemKind.Property);
                                item.detail = prop.signature;
                                item.documentation = new vscode.MarkdownString(
                                    `${prop.description}\n\n**Example:**\n\`\`\`rq\n${prop.example}\n\`\`\``
                                );
                                item.insertText = prop.name + ': ';
                                return item;
                            });
                    }

                    const suggestions: vscode.CompletionItem[] = [...builtinFunctionItems()];
                    try {
                        const variables = await cliService.listVariables(document.uri.fsPath, environmentProvider?.getSelectedEnvironment());
                        variables.forEach(v => {
                            const item = new vscode.CompletionItem(v.name, vscode.CompletionItemKind.Variable);
                            item.detail = v.value ? `= ${v.value}` : v.source;
                            item.insertText = v.name;
                            suggestions.push(item);
                        });
                    } catch { /* ignore */ }
                    ENDPOINT_PROPERTIES.forEach(prop => {
                        if (existingNamedEp.has(prop.name)) { return; }
                        const item = new vscode.CompletionItem(prop.name, vscode.CompletionItemKind.Property);
                        item.detail = prop.signature + ' (named)';
                        item.documentation = new vscode.MarkdownString(
                            `${prop.description}\n\n**Example:**\n\`\`\`rq\n${prop.example}\n\`\`\``
                        );
                        item.insertText = prop.name + ': ';
                        suggestions.push(item);
                    });
                    return suggestions;
                }
            }
            
            // Header key completion inside array literals
            const headerKeyMatch = linePrefix.match(/^\s*"?([a-zA-Z0-9_-]*)$/);
            if (headerKeyMatch) {
                const blockText = document.getText(new vscode.Range(
                    new vscode.Position(Math.max(0, position.line - 30), 0),
                    position
                ));
                if (insideArrayLiteral(blockText)) {
                    const partial = headerKeyMatch[1];
                    const hasOpenQuote = /["'][a-zA-Z0-9_-]*$/.test(linePrefix);
                    const afterLine = document.lineAt(position.line).text.substring(position.character);
                    const trailingKey = afterLine.match(/^([a-zA-Z0-9_-]*)/)?.[1] ?? '';
                    const trailingQuote = hasOpenQuote && afterLine.charAt(trailingKey.length) === '"' ? 1 : 0;
                    const replaceRange = new vscode.Range(
                        position.line, position.character - partial.length - (hasOpenQuote ? 1 : 0),
                        position.line, position.character + trailingKey.length + trailingQuote
                    );
                    return COMMON_HEADERS.map(header => {
                        const item = new vscode.CompletionItem(header, vscode.CompletionItemKind.Value);
                        item.detail = 'HTTP header';
                        item.insertText = new vscode.SnippetString(`"${header}": "\${1:}"`);
                        item.range = replaceRange;
                        return item;
                    });
                }
            }

            // Check if we're after "io."
            if (linePrefix.endsWith('io.')) {
                return IO_FUNCTIONS.map(func => {
                    const item = new vscode.CompletionItem(func.name, vscode.CompletionItemKind.Function);
                    item.detail = func.signature;
                    item.documentation = new vscode.MarkdownString(
                        `${func.description}\n\n**Parameters:**\n${func.parameters.map(p => `- ${p}`).join('\n')}`
                    );
                    item.insertText = new vscode.SnippetString(`${func.name}("\${1:path}")`);
                    return item;
                });
            }

            // Check if we're after "sys."
            if (linePrefix.endsWith('sys.')) {
                return SYSTEM_FUNCTIONS.map(func => {
                    const item = new vscode.CompletionItem(func.name, vscode.CompletionItemKind.Function);
                    item.detail = func.signature;
                    item.documentation = new vscode.MarkdownString(
                        `${func.description}\n\n**Parameters:**\n${func.parameters.map(p => `- ${p}`).join('\n')}`
                    );
                    item.insertText = new vscode.SnippetString(`${func.name}("\${1:path}")`);
                    return item;
                });
            }

            // Check if we're after "random."
            if (linePrefix.endsWith('random.')) {
                return RANDOM_FUNCTIONS.map(func => {
                    const item = new vscode.CompletionItem(func.name, vscode.CompletionItemKind.Function);
                    item.detail = func.signature;
                    item.documentation = new vscode.MarkdownString(
                        `${func.description}\n\n**Parameters:**\n${func.parameters.map(p => `- ${p}`).join('\n')}`
                    );
                    item.insertText = new vscode.SnippetString(`${func.name}();`);
                    return item;
                });
            }

            // Check if we're after "datetime."
            if (linePrefix.endsWith('datetime.')) {
                return DATETIME_FUNCTIONS.map(func => {
                    const item = new vscode.CompletionItem(func.name, vscode.CompletionItemKind.Function);
                    item.detail = func.signature;
                    item.documentation = new vscode.MarkdownString(
                        `${func.description}\n\n**Parameters:**\n${func.parameters.map(p => `- ${p}`).join('\n')}`
                    );
                    item.insertText = new vscode.SnippetString(`${func.name}(\${1:});`);
                    return item;
                });
            }

            // Suggest "sys" namespace
            const sysContext = document.getText(new vscode.Range(
                new vscode.Position(position.line, 0),
                position
            ));
            
            // Only suggest sys if we're in a context where it makes sense
            if (/\b(let\s+\w+\s*=\s*|body:\s*)$/.test(sysContext)) {
                const sysItem = new vscode.CompletionItem('sys', vscode.CompletionItemKind.Module);
                sysItem.detail = 'System namespace';
                sysItem.documentation = 'System functions for file operations and utilities';
                sysItem.insertText = new vscode.SnippetString('sys.');
                sysItem.command = {
                    command: 'editor.action.triggerSuggest',
                    title: 'Re-trigger completions'
                };
                
                const randomItem = new vscode.CompletionItem('random', vscode.CompletionItemKind.Module);
                randomItem.detail = 'Random namespace';
                randomItem.documentation = 'Random value generators';
                randomItem.insertText = new vscode.SnippetString('random.');
                randomItem.command = {
                    command: 'editor.action.triggerSuggest',
                    title: 'Re-trigger completions'
                };

                const datetimeItem = new vscode.CompletionItem('datetime', vscode.CompletionItemKind.Module);
                datetimeItem.detail = 'DateTime namespace';
                datetimeItem.documentation = 'DateTime functions';
                datetimeItem.insertText = new vscode.SnippetString('datetime.');
                datetimeItem.command = {
                    command: 'editor.action.triggerSuggest',
                    title: 'Re-trigger completions'
                };
                
                return [sysItem, randomItem, datetimeItem];
            }

            // Top-level keyword snippets — only on an empty/keyword-only line outside any block
            if (/^\s*(rq|ep|env|auth|import|let)?$/.test(linePrefix)) {
                const prevText = document.getText(new vscode.Range(new vscode.Position(0, 0), position));
                const insideBlock = !!rqMatch || !!epMatch || insideEnvOrAuthBlock(prevText);
                if (!insideBlock) {
                    const snippets: vscode.CompletionItem[] = [
                        (() => {
                            const i = new vscode.CompletionItem('rq', vscode.CompletionItemKind.Keyword);
                            i.detail = 'HTTP request';
                            i.insertText = new vscode.SnippetString('rq ${1:request_name}("${2:https://api.example.com/endpoint}");');
                            i.sortText = '0rq';
                            return i;
                        })(),
                        (() => {
                            const i = new vscode.CompletionItem('ep', vscode.CompletionItemKind.Keyword);
                            i.detail = 'Endpoint block';
                            i.insertText = new vscode.SnippetString('ep ${1:name}("${2:https://api.example.com}") {\n\trq ${3:request_name}("${4:path}");\n}');
                            i.sortText = '0ep';
                            return i;
                        })(),
                        (() => {
                            const i = new vscode.CompletionItem('env', vscode.CompletionItemKind.Keyword);
                            i.detail = 'Environment block';
                            i.insertText = new vscode.SnippetString('env ${1:local} {\n\t${2:api_url}: "${3:http://localhost:8080}"\n}');
                            i.sortText = '0env';
                            return i;
                        })(),
                        (() => {
                            const i = new vscode.CompletionItem('auth', vscode.CompletionItemKind.Keyword);
                            i.detail = 'Auth block';
                            i.insertText = new vscode.SnippetString('auth ${1:my_auth}(auth_type.${2}) {\n\t${3}\n}');
                            i.sortText = '0auth';
                            return i;
                        })(),
                        (() => {
                            const i = new vscode.CompletionItem('auth bearer', vscode.CompletionItemKind.Keyword);
                            i.detail = 'Auth block — Bearer Token';
                            i.insertText = new vscode.SnippetString('auth ${1:my_auth}(auth_type.bearer) {\n\ttoken: "${2:}"\n}');
                            i.sortText = '0auth_bearer';
                            return i;
                        })(),
                        (() => {
                            const i = new vscode.CompletionItem('auth oauth2_client_credentials (client_secret)', vscode.CompletionItemKind.Keyword);
                            i.detail = 'Auth block — OAuth2 Client Credentials (client secret)';
                            i.insertText = new vscode.SnippetString('auth ${1:my_auth}(auth_type.oauth2_client_credentials) {\n\tclient_id: "${2:}",\n\tclient_secret: "${3:}",\n\ttoken_url: "${4:}",\n\tscope: "${5:}"\n}');
                            i.sortText = '0auth_client_credentials_secret';
                            return i;
                        })(),
                        (() => {
                            const i = new vscode.CompletionItem('auth oauth2_client_credentials (cert_file)', vscode.CompletionItemKind.Keyword);
                            i.detail = 'Auth block — OAuth2 Client Credentials (certificate)';
                            i.insertText = new vscode.SnippetString('auth ${1:my_auth}(auth_type.oauth2_client_credentials) {\n\tclient_id: "${2:}",\n\tcert_file: "${3:}",\n\tcert_password: "${4:}",\n\ttoken_url: "${5:}",\n\tscope: "${6:}"\n}');
                            i.sortText = '0auth_client_credentials_cert';
                            return i;
                        })(),
                        (() => {
                            const i = new vscode.CompletionItem('auth oauth2_authorization_code', vscode.CompletionItemKind.Keyword);
                            i.detail = 'Auth block — OAuth2 Authorization Code with PKCE';
                            i.insertText = new vscode.SnippetString('auth ${1:my_auth}(auth_type.oauth2_authorization_code) {\n\tclient_id: "${2:}",\n\tauthorization_url: "${3:}",\n\ttoken_url: "${4:}",\n\tscope: "${5:}"\n}');
                            i.sortText = '0auth_authorization_code';
                            return i;
                        })(),
                        (() => {
                            const i = new vscode.CompletionItem('auth oauth2_implicit', vscode.CompletionItemKind.Keyword);
                            i.detail = 'Auth block — OAuth2 Implicit Flow';
                            i.insertText = new vscode.SnippetString('auth ${1:my_auth}(auth_type.oauth2_implicit) {\n\tclient_id: "${2:}",\n\tauthorization_url: "${3:}",\n\tscope: "${4:}"\n}');
                            i.sortText = '0auth_implicit';
                            return i;
                        })(),
                        (() => {
                            const i = new vscode.CompletionItem('import', vscode.CompletionItemKind.Keyword);
                            i.detail = 'Import file';
                            i.insertText = new vscode.SnippetString('import "${1:file}";');
                            i.sortText = '0import';
                            return i;
                        })(),
                    ];
                    return snippets;
                }
            }

            return undefined;
        }
    },
    '.', '{', '[', ',', ' ', 'v', 'e', 'p', 'q', ')', '<', '=', '"', ':', '(', '\n'
);
