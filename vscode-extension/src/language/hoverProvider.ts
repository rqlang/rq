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

export const hoverProvider = vscode.languages.registerHoverProvider('rq', {
    async provideHover(document: vscode.TextDocument, position: vscode.Position) {
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
                    return undefined;
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
