import * as vscode from 'vscode';
import * as cliService from '../cliService';
import {
    SYSTEM_FUNCTIONS,
    IO_FUNCTIONS,
    REQUEST_PROPERTIES,
    ENDPOINT_PROPERTIES,
    parseVariables
} from './definitions';

let environmentProvider: { getSelectedEnvironment(): string | undefined } | undefined;

export function setEnvironmentProvider(provider: { getSelectedEnvironment(): string | undefined }) {
    environmentProvider = provider;
}

function resolveRequestMethod(document: vscode.TextDocument, lineNum: number, name: string): string {
    const httpMethods = ['get', 'post', 'put', 'delete', 'patch', 'head', 'options'];
    if (httpMethods.includes(name.toLowerCase())) {
        return name.toUpperCase();
    }
    for (let i = lineNum - 1; i >= Math.max(0, lineNum - 5); i--) {
        const prevLine = document.lineAt(i).text.trim();
        const methodMatch = /\[method\((\w+)\)\]/.exec(prevLine);
        if (methodMatch) {
            return methodMatch[1].toUpperCase();
        }
        if (!/^\[/.test(prevLine) && prevLine.length > 0) {
            break;
        }
    }
    return 'GET';
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

function parseEnvVariables(document: vscode.TextDocument, startLine: number): string[] {
    const vars: string[] = [];
    let depth = 0;
    for (let i = startLine; i < document.lineCount; i++) {
        const line = document.lineAt(i).text;
        let inString = false;
        let stringChar = '';
        for (const ch of line) {
            if (inString) {
                if (ch === stringChar) { inString = false; }
            } else if (ch === '"' || ch === "'") {
                inString = true;
                stringChar = ch;
            } else if (ch === '{') {
                depth++;
            } else if (ch === '}') {
                depth--;
                if (depth <= 0) { return vars; }
            }
        }
        const varMatch = /^\s+(\w+)\s*:/.exec(line);
        if (varMatch && depth > 0) {
            vars.push(varMatch[1]);
        }
    }
    return vars;
}

export const hoverProvider = vscode.languages.registerHoverProvider('rq', {
    async provideHover(document: vscode.TextDocument, position: vscode.Position) {
        const lineText = document.lineAt(position.line).text;
        const col = position.character;

        // Check for rq declaration hover (cursor on keyword or name)
        const rqDeclMatch = /^\s*(rq)\s+(\w+)\s*\(([^)]*)/.exec(lineText);
        if (rqDeclMatch) {
            const keywordStart = lineText.indexOf(rqDeclMatch[1]);
            const nameEnd = lineText.indexOf('(');
            if (col >= keywordStart && col < nameEnd) {
                const name = rqDeclMatch[2];
                const argsText = rqDeclMatch[3];
                const method = resolveRequestMethod(document, position.line, name);
                const urlMatch = /"([^"]*)"/.exec(argsText);
                const url = urlMatch ? urlMatch[1] : undefined;
                const contents = new vscode.MarkdownString();
                contents.appendMarkdown(`**Request: \`${name}\`**\n\n`);
                contents.appendCodeblock(`[${method}]\nrq ${name}(${url ? `"${url}"` : '...'})`, 'rq');
                if (url) {
                    contents.appendMarkdown(`\n**URL:** \`${url}\``);
                }
                return new vscode.Hover(contents);
            }
        }

        // Check for ep declaration hover (cursor on keyword or name)
        const epDeclMatch = /^\s*(ep)\s+(\w+)(?:<(\w+)>)?\s*[\({]/.exec(lineText);
        if (epDeclMatch) {
            const keywordStart = lineText.indexOf(epDeclMatch[1]);
            const openParen = lineText.search(/[\({]/);
            if (col >= keywordStart && col < openParen) {
                const name = epDeclMatch[2];
                const parent = epDeclMatch[3];
                const argsText = lineText.slice(openParen + 1);
                const urlMatch = /"([^"]*)"/.exec(argsText);
                const url = urlMatch ? urlMatch[1] : undefined;
                let sig = `ep ${name}`;
                if (parent) { sig += `<${parent}>`; }
                sig += `(${url ? `"${url}"` : '...'})`;
                const contents = new vscode.MarkdownString();
                contents.appendMarkdown(`**Endpoint: \`${name}\`**\n\n`);
                contents.appendCodeblock(sig, 'rq');
                if (parent) { contents.appendMarkdown(`\n**Extends:** \`${parent}\``); }
                if (url) { contents.appendMarkdown(`\n**Base URL:** \`${url}\``); }
                return new vscode.Hover(contents);
            }
        }

        // Check for auth declaration hover (cursor on keyword or name)
        const authDeclMatch = /^\s*(auth)\s+(\w+)\s*\(auth_type\.(\w+)\)/.exec(lineText);
        if (authDeclMatch) {
            const keywordStart = lineText.indexOf(authDeclMatch[1]);
            const openParen = lineText.indexOf('(');
            if (col >= keywordStart && col < openParen) {
                const name = authDeclMatch[2];
                const authType = authDeclMatch[3];
                const contents = new vscode.MarkdownString();
                contents.appendMarkdown(`**Auth: \`${name}\`**\n\n`);
                contents.appendCodeblock(`auth ${name}(auth_type.${authType})`, 'rq');
                contents.appendMarkdown(`\n**Type:** ${formatAuthType(authType)}`);
                return new vscode.Hover(contents);
            }
        }

        // Check for env declaration hover (cursor on keyword or name)
        const envDeclMatch = /^\s*(env)\s+(\w+)\s*\{/.exec(lineText);
        if (envDeclMatch) {
            const keywordStart = lineText.indexOf(envDeclMatch[1]);
            const bracePos = lineText.indexOf('{');
            if (col >= keywordStart && col < bracePos) {
                const name = envDeclMatch[2];
                const vars = parseEnvVariables(document, position.line);
                const contents = new vscode.MarkdownString();
                contents.appendMarkdown(`**Environment: \`${name}\`**\n\n`);
                contents.appendCodeblock(`env ${name} { ... }`, 'rq');
                if (vars.length > 0) {
                    const preview = vars.slice(0, 5).map(v => `\`${v}\``).join(', ');
                    const more = vars.length > 5 ? ` *(+${vars.length - 5} more)*` : '';
                    contents.appendMarkdown(`\n**Variables:** ${preview}${more}`);
                }
                return new vscode.Hover(contents);
            }
        }

        // Check for endpoint properties (url, headers, qs)
        const epPropRange = document.getWordRangeAtPosition(position, /\b(url|headers|qs)\b/);
        if (epPropRange) {
            const word = document.getText(epPropRange);
            
            // Check if we're in an ep context
            const surroundingText = document.getText(new vscode.Range(
                new vscode.Position(Math.max(0, position.line - 5), 0),
                position
            ));
            
            if (/\bep\s+\w+\s*\(/.test(surroundingText)) {
                const prop = ENDPOINT_PROPERTIES.find(p => p.name === word);
                if (prop) {
                    const contents = new vscode.MarkdownString();
                    contents.appendMarkdown(`**Endpoint Property: \`${prop.name}\`**\n\n`);
                    contents.appendCodeblock(prop.signature, 'rq');
                    contents.appendMarkdown(`\n${prop.description}\n\n`);
                    contents.appendMarkdown('**Example:**\n');
                    contents.appendCodeblock(prop.example, 'rq');
                    return new vscode.Hover(contents);
                }
            }
        }
        
        // Check for request properties (url, headers, body, method)
        const propRange = document.getWordRangeAtPosition(position, /\b(url|headers|body|method)\b/);
        if (propRange) {
            const word = document.getText(propRange);
            const prop = REQUEST_PROPERTIES.find(p => p.name === word);
            
            if (prop) {
                // Verify we're in an rq context by checking the surrounding text
                if (/\brq\s+\w+\s*\(/.test(document.getText(new vscode.Range(
                    new vscode.Position(Math.max(0, position.line - 5), 0),
                    position
                )))) {
                    const contents = new vscode.MarkdownString();
                    contents.appendMarkdown(`**Request Property: \`${prop.name}\`**\n\n`);
                    contents.appendCodeblock(prop.signature, 'rq');
                    contents.appendMarkdown(`\n${prop.description}\n\n`);
                    contents.appendMarkdown('**Example:**\n');
                    contents.appendCodeblock(prop.example, 'rq');
                    return new vscode.Hover(contents);
                }
            }
        }
        
        // Check for io functions
        const ioRange = document.getWordRangeAtPosition(position, /io\.\w+/);
        if (ioRange) {
            const word = document.getText(ioRange);
            const funcName = word.replace('io.', '');
            const func = IO_FUNCTIONS.find(f => f.name === funcName);
            
            if (func) {
                const contents = new vscode.MarkdownString();
                contents.appendCodeblock(func.signature, 'rq');
                contents.appendMarkdown(`\n${func.description}\n\n`);
                contents.appendMarkdown('**Parameters:**\n');
                func.parameters.forEach(p => {
                    contents.appendMarkdown(`- ${p}\n`);
                });
                return new vscode.Hover(contents);
            }
        }

        // Check for system functions
        const sysRange = document.getWordRangeAtPosition(position, /sys\.\w+/);
        if (sysRange) {
            const word = document.getText(sysRange);
            const funcName = word.replace('sys.', '');
            const func = SYSTEM_FUNCTIONS.find(f => f.name === funcName);
            
            if (func) {
                const contents = new vscode.MarkdownString();
                contents.appendCodeblock(func.signature, 'rq');
                contents.appendMarkdown(`\n${func.description}\n\n`);
                contents.appendMarkdown('**Parameters:**\n');
                func.parameters.forEach(p => {
                    contents.appendMarkdown(`- ${p}\n`);
                });
                return new vscode.Hover(contents);
            }
        }
        
        // Check for variable references (in {{ }} or standalone)
        const varRange = document.getWordRangeAtPosition(position, /[a-zA-Z_][a-zA-Z0-9_]*/);
        if (varRange) {
            const word = document.getText(varRange);

            const artifactDeclMatch = /^\s*(?:rq|ep|auth|env)\s+(\w+)/.exec(lineText);
            if (artifactDeclMatch && artifactDeclMatch[1] === word) {
                return undefined;
            }

            const environment = environmentProvider?.getSelectedEnvironment();

            if (environment) {
                try {
                    const sourceDirectory = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
                    const result = await cliService.showVariable(word, sourceDirectory, environment);
                    const contents = new vscode.MarkdownString();
                    contents.appendMarkdown(`**Variable: \`${result.name}\`** *(${result.source})*\n\n`);
                    contents.appendMarkdown('**Value:**\n');
                    contents.appendCodeblock(result.value, 'rq');
                    return new vscode.Hover(contents);
                } catch {
                    // fall through to local variable hover
                }
            }

            const variables = parseVariables(document);
            const variable = variables.find(v => v.name === word);
            if (variable) {
                const contents = new vscode.MarkdownString();
                contents.appendMarkdown(`**Variable: \`${variable.name}\`**\n\n`);
                contents.appendMarkdown(`Defined on line ${variable.line + 1}\n\n`);
                contents.appendMarkdown('**Value:**\n');
                contents.appendCodeblock(variable.value, 'rq');
                return new vscode.Hover(contents);
            }
        }

        return undefined;
    }
});
