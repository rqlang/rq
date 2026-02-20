import * as vscode from 'vscode';
import { 
    SYSTEM_FUNCTIONS, 
    IO_FUNCTIONS,
    RANDOM_FUNCTIONS,
    DATETIME_FUNCTIONS,
    REQUEST_PROPERTIES, 
    ENDPOINT_PROPERTIES, 
    parseVariables 
} from './definitions';

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

export const completionProvider = vscode.languages.registerCompletionItemProvider(
    'rq',
    {
        async provideCompletionItems(document: vscode.TextDocument, position: vscode.Position) {
            const linePrefix = document.lineAt(position).text.substr(0, position.character);

            // Import directive completion without duplicating the keyword
            // Cases:
            //   1) User typed 'import' (no trailing space) -> suggest full snippet including keyword.
            //   2) User typed 'import ' (has trailing space) -> only insert the path + quotes/semicolon.
            const importFullMatch = /^\s*import$/; // exact 'import'
            const importSpaceMatch = /^\s*import\s+$/; // 'import ' with trailing space(s)
            if (importFullMatch.test(linePrefix) || importSpaceMatch.test(linePrefix)) {
                // Avoid offering inside rq(...) or ep(...)
                const prevText = document.getText(new vscode.Range(new vscode.Position(0, 0), position));
                const inRqOrEp = /\b(rq|ep)\s+\w+\s*\(/.test(prevText);
                if (!inRqOrEp) {
                    const importItem = new vscode.CompletionItem('import file', vscode.CompletionItemKind.Snippet);
                    importItem.detail = 'Import another .rq file';
                    importItem.documentation = new vscode.MarkdownString('Inline another RQ file at the top of this file.');
                    // Decide snippet variant based on whether keyword already typed followed by space
                    if (importSpaceMatch.test(linePrefix)) {
                        // User already typed 'import ' so just add the quoted path & semicolon
                        importItem.insertText = new vscode.SnippetString('"${1:path.rq}";');
                    } else {
                        // No trailing space yet; provide full directive
                        importItem.insertText = new vscode.SnippetString(' import "${1:path.rq}";');
                    }
                    importItem.commitCharacters = [';'];
                    return [importItem];
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
                    const strItem = new vscode.CompletionItem('let string', vscode.CompletionItemKind.Snippet);
                    strItem.detail = 'String variable';
                    strItem.documentation = new vscode.MarkdownString('Declare a string variable');
                    strItem.insertText = new vscode.SnippetString(`${prefixForNewDecl}\${1:name} = "\${2:value}"`);
                    if (replacementRange) { strItem.range = replacementRange; }
                    variants.push(strItem);

                    // 2. JSON object variable
                    const jsonItem = new vscode.CompletionItem('let json', vscode.CompletionItemKind.Snippet);
                    jsonItem.detail = 'JSON object variable';
                    jsonItem.documentation = new vscode.MarkdownString('Declare a JSON object variable');
                    jsonItem.insertText = new vscode.SnippetString(`${prefixForNewDecl}\${1:name} = {\n    "\${2:key}": "\${3:value}"\n}`);
                    if (replacementRange) { jsonItem.range = replacementRange; }
                    variants.push(jsonItem);

                    // 3. Request URL variable
                    const urlItem = new vscode.CompletionItem('let url', vscode.CompletionItemKind.Snippet);
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
                
                // Determine if we're using named or unnamed parameters
                // Named parameters have "property:" pattern
                const hasNamedParams = /\b(url|headers|body|method)\s*:/.test(matchedText);
                
                // Count commas in the matched text to determine position
                // More reliable: just count all commas not inside nested structures
                let commaCount = 0;
                let depth = 0;
                let inString = false;
                let stringChar = '';
                
                for (let i = 0; i < matchedText.length; i++) {
                    const char = matchedText[i];
                    const prevChar = i > 0 ? matchedText[i - 1] : '';
                    
                    // Track string boundaries
                    if ((char === '"' || char === "'") && prevChar !== '\\') {
                        if (!inString) {
                            inString = true;
                            stringChar = char;
                        } else if (char === stringChar) {
                            inString = false;
                        }
                    }
                    
                    // Track bracket/brace/paren depth
                    if (!inString) {
                        if (char === '(' || char === '[' || char === '{') {
                            depth++;
                        } else if (char === ')' || char === ']' || char === '}') {
                            depth--;
                        } else if (char === ',' && depth === 1) {
                            // depth === 1 means we're inside the rq(...) but not in nested structures
                            commaCount++;
                        }
                    }
                }
                
                const unnamedParamCount = commaCount;
                
                // Check if we should suggest properties
                const atStartOfParams = /\brq\s+\w+\s*\(\s*$/.test(linePrefix);
                const afterComma = /,\s*$/.test(linePrefix);
                const onNewLine = /^\s*$/.test(linePrefix) && rqMatch;
                
                if (atStartOfParams || afterComma || onNewLine) {
                    const existingNamed = collectRqNamed(matchedText);
                    // If we have named parameters already, only suggest those not yet used
                    if (hasNamedParams) {
                        return REQUEST_PROPERTIES
                            .filter(p => !existingNamed.has(p.name))
                            .map(prop => {
                                const item = new vscode.CompletionItem(prop.name, vscode.CompletionItemKind.Property);
                                item.detail = prop.signature;
                                item.documentation = new vscode.MarkdownString(
                                    `${prop.description}\n\n**Example:**\n\`\`\`rq\n${prop.example}\n\`\`\``
                                );
                                if (prop.name === 'headers') {
                                    item.insertText = new vscode.SnippetString('headers: ["${1:key}": "${2:value}"]');
                                } else if (prop.name === 'body') {
                                    item.insertText = new vscode.SnippetString('body: \\${${1:}}');
                                } else {
                                    item.insertText = new vscode.SnippetString(prop.name + ': "${1:value}"');
                                }
                                return item;
                            });
                    }
                    
                    // If we're at the start with no named params, suggest both styles
                    if (atStartOfParams && !hasNamedParams) {
                        const suggestions: vscode.CompletionItem[] = [];
                        
                        // Suggest unnamed URL parameter (first parameter)
                        const urlUnnamed = new vscode.CompletionItem('"url"', vscode.CompletionItemKind.Value);
                        urlUnnamed.detail = 'Unnamed URL parameter (position 1)';
                        urlUnnamed.documentation = new vscode.MarkdownString(
                            'First unnamed parameter is the URL\n\n**Example:**\n```rq\nrq name("https://api.example.com")\n```'
                        );
                        urlUnnamed.insertText = new vscode.SnippetString('"\${1:https://api.example.com}"');
                        suggestions.push(urlUnnamed);
                        
                        // Also suggest named properties
                        REQUEST_PROPERTIES.forEach(prop => {
                            if (existingNamed.has(prop.name)) {return;} // skip duplicates
                            const item = new vscode.CompletionItem(prop.name, vscode.CompletionItemKind.Property);
                            item.detail = prop.signature + ' (named)';
                            item.documentation = new vscode.MarkdownString(
                                `${prop.description}\n\n**Example:**\n\`\`\`rq\n${prop.example}\n\`\`\``
                            );
                            if (prop.name === 'headers') {
                                item.insertText = new vscode.SnippetString('headers: ["${1:key}": "${2:value}"]');
                            } else if (prop.name === 'body') {
                                item.insertText = new vscode.SnippetString('body: \\${${1:}}');
                            } else {
                                item.insertText = new vscode.SnippetString(prop.name + ': "${1:value}"');
                            }
                            suggestions.push(item);
                        });
                        
                        return suggestions;
                    }
                    
                    // After comma without named params - suggest what comes next
                    if (afterComma && !hasNamedParams) {
                        const suggestions: vscode.CompletionItem[] = [];
                        
                        // After first comma: suggest headers array (position 2)
                        if (unnamedParamCount === 1) {
                            const headersUnnamed = new vscode.CompletionItem('[ ]', vscode.CompletionItemKind.Value);
                            headersUnnamed.detail = 'Unnamed headers array (position 2)';
                            headersUnnamed.documentation = new vscode.MarkdownString(
                                'Second unnamed parameter is the headers array\n\n**Example:**\n```rq\nrq name("url", ["Content-Type": "application/json"])\n```'
                            );
                            headersUnnamed.insertText = new vscode.SnippetString('["\${1:key}": "\${2:value}"]');
                            headersUnnamed.sortText = '0'; // Make it appear first
                            suggestions.push(headersUnnamed);
                        }
                        
                        // After second comma: suggest body (position 3)
                        if (unnamedParamCount === 2) {
                            const bodyUnnamed = new vscode.CompletionItem('${ }', vscode.CompletionItemKind.Value);
                            bodyUnnamed.detail = 'Unnamed body JSON object (position 3)';
                            bodyUnnamed.documentation = new vscode.MarkdownString(
                                'Third unnamed parameter is the request body as JSON (must start with $)\n\n**Examples:**\n```rq\nrq name("url", headers, ${"key": "value"})\nrq name("url", headers, "string body")\n```'
                            );
                            bodyUnnamed.insertText = new vscode.SnippetString('\\${\${1:}}');
                            bodyUnnamed.sortText = '0';
                            suggestions.push(bodyUnnamed);
                            
                            const bodyStringUnnamed = new vscode.CompletionItem('"string"', vscode.CompletionItemKind.Value);
                            bodyStringUnnamed.detail = 'Unnamed body string (position 3)';
                            bodyStringUnnamed.documentation = new vscode.MarkdownString(
                                'Third unnamed parameter as a string\n\n**Example:**\n```rq\nrq name("url", headers, "body content")\n```'
                            );
                            bodyStringUnnamed.insertText = new vscode.SnippetString('"\${1:body}"');
                            bodyStringUnnamed.sortText = '1';
                            suggestions.push(bodyStringUnnamed);
                        }
                        
                        // Can switch to named parameters at any point
                        REQUEST_PROPERTIES.forEach(prop => {
                            if (existingNamed.has(prop.name)) {return;}
                            const item = new vscode.CompletionItem(prop.name, vscode.CompletionItemKind.Property);
                            item.detail = prop.signature + ' (named)';
                            item.documentation = new vscode.MarkdownString(
                                `Switch to named parameters\n\n${prop.description}\n\n**Example:**\n\`\`\`rq\n${prop.example}\n\`\`\``
                            );
                            if (prop.name === 'headers') {
                                item.insertText = new vscode.SnippetString('headers: ["${1:key}": "${2:value}"]');
                            } else if (prop.name === 'body') {
                                item.insertText = new vscode.SnippetString('body: \\${${1:}}');
                            } else {
                                item.insertText = new vscode.SnippetString(prop.name + ': "${1:value}"');
                            }
                            suggestions.push(item);
                        });
                        
                        return suggestions;
                    }
                }
            }
            
            // Check if we're inside an ep (endpoint) declaration
            const epMatch = textBeforeCursor.match(/\bep\s+\w+\s*\([^{]*$/s);
            if (epMatch) {
                const matchedText = epMatch[0];
                
                // Determine if we're using named or unnamed parameters
                const hasNamedParams = /\b(url|headers|qs)\s*:/.test(matchedText);
                
                // Count commas to determine position
                let commaCount = 0;
                let depth = 0;
                let inString = false;
                let stringChar = '';
                
                for (let i = 0; i < matchedText.length; i++) {
                    const char = matchedText[i];
                    const prevChar = i > 0 ? matchedText[i - 1] : '';
                    
                    // Track string boundaries
                    if ((char === '"' || char === "'") && prevChar !== '\\') {
                        if (!inString) {
                            inString = true;
                            stringChar = char;
                        } else if (char === stringChar) {
                            inString = false;
                        }
                    }
                    
                    // Track bracket/brace/paren depth
                    if (!inString) {
                        if (char === '(' || char === '[' || char === '{') {
                            depth++;
                        } else if (char === ')' || char === ']' || char === '}') {
                            depth--;
                        } else if (char === ',' && depth === 1) {
                            commaCount++;
                        }
                    }
                }
                
                const unnamedParamCount = commaCount;
                
                // Check if we should suggest properties
                const atStartOfParams = /\bep\s+\w+\s*\(\s*$/.test(linePrefix);
                const afterComma = /,\s*$/.test(linePrefix);
                const onNewLine = /^\s*$/.test(linePrefix) && epMatch;
                
                if (atStartOfParams || afterComma || onNewLine) {
                    const existingNamedEp = collectEpNamed(matchedText);
                    if (hasNamedParams) {
                        return ENDPOINT_PROPERTIES
                            .filter(p => !existingNamedEp.has(p.name))
                            .map(prop => {
                                const item = new vscode.CompletionItem(prop.name, vscode.CompletionItemKind.Property);
                                item.detail = prop.signature;
                                item.documentation = new vscode.MarkdownString(
                                    `${prop.description}\n\n**Example:**\n\`\`\`rq\n${prop.example}\n\`\`\``
                                );
                                if (prop.name === 'headers') {
                                    item.insertText = new vscode.SnippetString('headers: ["${1:key}": "${2:value}"]');
                                } else if (prop.name === 'qs') {
                                    item.insertText = new vscode.SnippetString('qs: "${1:param}=${2:value}"');
                                } else {
                                    item.insertText = new vscode.SnippetString(prop.name + ': "${1:value}"');
                                }
                                return item;
                            });
                    }
                    
                    // If we're at the start with no named params, suggest both styles
                    if (atStartOfParams && !hasNamedParams) {
                        const suggestions: vscode.CompletionItem[] = [];
                        
                        // Suggest unnamed URL parameter (first parameter)
                        const urlUnnamed = new vscode.CompletionItem('"url"', vscode.CompletionItemKind.Value);
                        urlUnnamed.detail = 'Unnamed URL parameter (position 1)';
                        urlUnnamed.documentation = new vscode.MarkdownString(
                            'First unnamed parameter is the base URL\n\n**Example:**\n```rq\nep api("https://api.example.com") { }\n```'
                        );
                        urlUnnamed.insertText = new vscode.SnippetString('"\${1:https://api.example.com}"');
                        suggestions.push(urlUnnamed);
                        
                        // Also suggest named properties
                        ENDPOINT_PROPERTIES.forEach(prop => {
                            if (existingNamedEp.has(prop.name)) {return;}
                            const item = new vscode.CompletionItem(prop.name, vscode.CompletionItemKind.Property);
                            item.detail = prop.signature + ' (named)';
                            item.documentation = new vscode.MarkdownString(
                                `${prop.description}\n\n**Example:**\n\`\`\`rq\n${prop.example}\n\`\`\``
                            );
                            if (prop.name === 'headers') {
                                item.insertText = new vscode.SnippetString('headers: ["${1:key}": "${2:value}"]');
                            } else if (prop.name === 'qs') {
                                item.insertText = new vscode.SnippetString('qs: "${1:param}=${2:value}"');
                            } else {
                                item.insertText = new vscode.SnippetString(prop.name + ': "${1:value}"');
                            }
                            suggestions.push(item);
                        });
                        
                        return suggestions;
                    }
                    
                    // After comma without named params - suggest what comes next
                    if (afterComma && !hasNamedParams) {
                        const suggestions: vscode.CompletionItem[] = [];
                        
                        // After first comma: suggest headers array (position 2)
                        if (unnamedParamCount === 1) {
                            const headersUnnamed = new vscode.CompletionItem('[ ]', vscode.CompletionItemKind.Value);
                            headersUnnamed.detail = 'Unnamed headers array (position 2)';
                            headersUnnamed.documentation = new vscode.MarkdownString(
                                'Second unnamed parameter is the headers array\n\n**Example:**\n```rq\nep api("url", ["Authorization": "Bearer token"]) { }\n```'
                            );
                            headersUnnamed.insertText = new vscode.SnippetString('["\${1:key}": "\${2:value}"]');
                            headersUnnamed.sortText = '0';
                            suggestions.push(headersUnnamed);
                        }
                        
                        // After second comma: suggest query string (position 3)
                        if (unnamedParamCount === 2) {
                            const qsUnnamed = new vscode.CompletionItem('"..."', vscode.CompletionItemKind.Value);
                            qsUnnamed.detail = 'Unnamed query string (position 3)';
                            qsUnnamed.documentation = new vscode.MarkdownString(
                                'Third unnamed parameter is the query string\n\n**Example:**\n```rq\nep api("url", headers, "version=1") { }\n```'
                            );
                            qsUnnamed.insertText = new vscode.SnippetString('"\${1:param}=\${2:value}"');
                            qsUnnamed.sortText = '0';
                            suggestions.push(qsUnnamed);
                        }
                        
                        // Can switch to named parameters at any point
                        ENDPOINT_PROPERTIES.forEach(prop => {
                            if (existingNamedEp.has(prop.name)) {return;}
                            const item = new vscode.CompletionItem(prop.name, vscode.CompletionItemKind.Property);
                            item.detail = prop.signature + ' (named)';
                            item.documentation = new vscode.MarkdownString(
                                `Switch to named parameters\n\n${prop.description}\n\n**Example:**\n\`\`\`rq\n${prop.example}\n\`\`\``
                            );
                            if (prop.name === 'headers') {
                                item.insertText = new vscode.SnippetString('headers: ["${1:key}": "${2:value}"]');
                            } else if (prop.name === 'qs') {
                                item.insertText = new vscode.SnippetString('qs: "${1:param}=${2:value}"');
                            } else {
                                item.insertText = new vscode.SnippetString(prop.name + ': "${1:value}"');
                            }
                            suggestions.push(item);
                        });
                        
                        return suggestions;
                    }
                }
            }
            
            // Check if we're inside {{ }} for variable interpolation
            const line = document.lineAt(position).text;
            const beforeCursor = line.substring(0, position.character);
            const afterCursor = line.substring(position.character);
            
            // Check if we're between {{ and }}
            const lastOpenBrace = beforeCursor.lastIndexOf('{{');
            const lastCloseBrace = beforeCursor.lastIndexOf('}}');
            const nextCloseBrace = afterCursor.indexOf('}}');
            
            if (lastOpenBrace > lastCloseBrace && nextCloseBrace !== -1) {
                // We're inside {{ }}, suggest variables
                const variables = parseVariables(document);
                
                return variables.map(variable => {
                    const item = new vscode.CompletionItem(variable.name, vscode.CompletionItemKind.Variable);
                    item.detail = `Variable (line ${variable.line + 1})`;
                    item.documentation = new vscode.MarkdownString(`Value: \`${variable.value}\``);
                    item.insertText = variable.name;
                    return item;
                });
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

            return undefined;
        }
    },
    '.', // Trigger completion after dot
    '{', // Trigger completion after opening brace
    ',', // Trigger completion after comma (for rq properties)
    ' ', // Trigger completion after space (for rq/env/ep properties)
    'v', // Trigger completion after final letter of 'env'
    'e', // Trigger completion after starting 'env'
    'p', // Trigger completion while typing 'ep'
    'q'  // Trigger completion while typing 'rq'
    ,')' // Trigger completion after closing parenthesis for rq semicolon suggestion
);
