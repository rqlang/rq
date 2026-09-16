import * as vscode from 'vscode';
import * as rqClient from '../rqClient';
import {
    SYSTEM_FUNCTIONS,
    IO_FUNCTIONS,
    RANDOM_FUNCTIONS,
    DATETIME_FUNCTIONS,
    FunctionDefinition,
    REQUEST_PROPERTIES,
    ENDPOINT_PROPERTIES,
    parseVariables,
    Variable,
    findRequiredAttributeLineInScope,
    isRequiredAttributeFor,
    declarationPattern,
    ATTRIBUTE_DEFINITIONS
} from './definitions';
import {
    buildRequestHover,
    buildEndpointHover,
    buildAuthHover,
    buildEnvironmentHover,
    buildImportHover,
    buildAttributeHover
} from './artifactHover';

let environmentProvider: { getSelectedEnvironment(): string | undefined } | undefined;

export function setEnvironmentProvider(provider: { getSelectedEnvironment(): string | undefined }) {
    environmentProvider = provider;
}

function buildRequiredVariableHover(name: string): vscode.MarkdownString {
    const contents = new vscode.MarkdownString();
    contents.appendMarkdown(`**Variable: \`${name}\`** *(required)*\n\n`);
    contents.appendMarkdown('Value provided at runtime.');
    return contents;
}

function buildResolvedVariableHover(entry: rqClient.VariableShowOutput): vscode.MarkdownString {
    if (entry.source === 'required') {
        return buildRequiredVariableHover(entry.name);
    }
    const contents = new vscode.MarkdownString();
    contents.appendMarkdown(`**Variable: \`${entry.name}\`** *(${entry.source})*\n\n`);
    contents.appendMarkdown('**Value:**\n');
    contents.appendCodeblock(entry.value, 'rq');
    return contents;
}

function buildLocalVariableHover(variable: Variable): vscode.MarkdownString {
    const contents = new vscode.MarkdownString();
    contents.appendMarkdown(`**Variable: \`${variable.name}\`**\n\n`);
    contents.appendCodeblock(`let ${variable.name} = ${variable.value};`, 'rq');
    contents.appendMarkdown(`\nDefined on line ${variable.line + 1}`);
    return contents;
}

function buildFunctionHover(func: FunctionDefinition): vscode.MarkdownString {
    const contents = new vscode.MarkdownString();
    contents.appendCodeblock(func.signature, 'rq');
    contents.appendMarkdown(`\n${func.description}\n\n`);
    if (func.parameters.length > 0) {
        contents.appendMarkdown('**Parameters:**\n');
        func.parameters.forEach(p => contents.appendMarkdown(`- ${p}\n`));
    }
    return contents;
}

export const hoverProvider = vscode.languages.registerHoverProvider('rq', {
    async provideHover(document: vscode.TextDocument, position: vscode.Position) {
        const lineText = document.lineAt(position.line).text;
        const col = position.character;

        const attributeMatch = /^\s*\[\s*([a-zA-Z_]\w*)/.exec(lineText);
        if (attributeMatch) {
            const nameStart = lineText.indexOf(attributeMatch[1]);
            if (col >= nameStart && col <= nameStart + attributeMatch[1].length) {
                const definition = ATTRIBUTE_DEFINITIONS.find(a => a.name === attributeMatch[1]);
                if (definition) { return new vscode.Hover(buildAttributeHover(definition)); }
            }
        }

        const importMatch = /^\s*(import)\s+"([^"]*)"/.exec(lineText);
        if (importMatch) {
            const keywordStart = lineText.indexOf(importMatch[1]);
            if (col >= keywordStart) {
                return new vscode.Hover(buildImportHover(importMatch[2]));
            }
        }

        // Check for rq declaration hover (cursor on keyword or name)
        const rqDeclMatch = /^\s*(rq)\s+([a-zA-Z_][a-zA-Z0-9_-]*)\s*\(/.exec(lineText);
        if (rqDeclMatch) {
            const keywordStart = lineText.indexOf(rqDeclMatch[1]);
            const openParen = lineText.indexOf('(');
            if (col >= keywordStart && col < openParen) {
                return new vscode.Hover(buildRequestHover(document, position.line, rqDeclMatch[2], openParen));
            }
        }

        // Check for ep declaration hover (cursor on keyword or name)
        const epDeclMatch = /^\s*(ep)\s+([a-zA-Z_][a-zA-Z0-9_-]*)\s*(?:<\s*([a-zA-Z_][a-zA-Z0-9_-]*)\s*>)?\s*[({]/.exec(lineText);
        if (epDeclMatch) {
            const keywordStart = lineText.indexOf(epDeclMatch[1]);
            const openIndex = lineText.search(/[({]/);
            if (col >= keywordStart && col < openIndex) {
                return new vscode.Hover(buildEndpointHover(document, position.line, epDeclMatch[2], epDeclMatch[3], openIndex));
            }
        }

        // Check for auth declaration hover (cursor on keyword or name)
        const authDeclMatch = /^\s*(auth)\s+([a-zA-Z_][a-zA-Z0-9_-]*)\s*\(auth_type\.(\w+)\)/.exec(lineText);
        if (authDeclMatch) {
            const keywordStart = lineText.indexOf(authDeclMatch[1]);
            const openParen = lineText.indexOf('(');
            if (col >= keywordStart && col < openParen) {
                return new vscode.Hover(buildAuthHover(document, position.line, authDeclMatch[2], authDeclMatch[3]));
            }
        }

        // Check for env declaration hover (cursor on keyword or name)
        const envDeclMatch = /^\s*(env)\s+([a-zA-Z_][a-zA-Z0-9_-]*)\s*\{/.exec(lineText);
        if (envDeclMatch) {
            const keywordStart = lineText.indexOf(envDeclMatch[1]);
            const bracePos = lineText.indexOf('{');
            if (col >= keywordStart && col < bracePos) {
                return new vscode.Hover(buildEnvironmentHover(document, position.line, envDeclMatch[2]));
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
            
            if (new RegExp(`${declarationPattern('ep')}\\(`).test(surroundingText)) {
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
        
        const ioRange = document.getWordRangeAtPosition(position, /io\.\w+/);
        if (ioRange) {
            const func = IO_FUNCTIONS.find(f => f.name === document.getText(ioRange).replace('io.', ''));
            if (func) { return new vscode.Hover(buildFunctionHover(func)); }
        }

        const randomRange = document.getWordRangeAtPosition(position, /random\.\w+/);
        if (randomRange) {
            const func = RANDOM_FUNCTIONS.find(f => f.name === document.getText(randomRange).replace('random.', ''));
            if (func) { return new vscode.Hover(buildFunctionHover(func)); }
        }

        const datetimeRange = document.getWordRangeAtPosition(position, /datetime\.\w+/);
        if (datetimeRange) {
            const func = DATETIME_FUNCTIONS.find(f => f.name === document.getText(datetimeRange).replace('datetime.', ''));
            if (func) { return new vscode.Hover(buildFunctionHover(func)); }
        }

        const sysRange = document.getWordRangeAtPosition(position, /sys\.\w+/);
        if (sysRange) {
            const func = SYSTEM_FUNCTIONS.find(f => f.name === document.getText(sysRange).replace('sys.', ''));
            if (func) { return new vscode.Hover(buildFunctionHover(func)); }
        }
        
        // Check for variable references (in {{ }} or standalone)
        const varRange = document.getWordRangeAtPosition(position, /[a-zA-Z_][a-zA-Z0-9_]*/);
        if (varRange) {
            const word = document.getText(varRange);

            const artifactDeclMatch = /^\s*(?:rq|ep|auth|env)\s+([a-zA-Z_][a-zA-Z0-9_-]*)/.exec(lineText);
            if (artifactDeclMatch && artifactDeclMatch[1] === word) {
                return undefined;
            }

            if (isRequiredAttributeFor(lineText, word)
                || findRequiredAttributeLineInScope(document, position.line, word) !== -1) {
                return new vscode.Hover(buildRequiredVariableHover(word));
            }

            const environment = environmentProvider?.getSelectedEnvironment();
            const localVariable = parseVariables(document).find(v => v.name === word);

            if (!environment && localVariable) {
                return new vscode.Hover(buildLocalVariableHover(localVariable));
            }

            try {
                const sourceDirectory = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
                const result = await rqClient.showVariable(word, sourceDirectory, environment, true, document.uri.fsPath);
                return new vscode.Hover(buildResolvedVariableHover(result));
            } catch {
                if (localVariable) {
                    return new vscode.Hover(buildLocalVariableHover(localVariable));
                }
            }
        }

        return undefined;
    }
});
