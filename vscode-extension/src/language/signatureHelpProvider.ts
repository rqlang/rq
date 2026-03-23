import * as vscode from 'vscode';

const RQ_PARAM_NAMES = ['url', 'headers', 'body'];
const EP_PARAM_NAMES = ['url', 'headers', 'qs'];
const AUTH_PARAM_NAMES = ['auth_type'];

export function getActiveParam(innerText: string, paramNames: string[]): number {
    let lastCommaPos = -1;
    let commaCount = 0;
    let depth = 0;
    let inString = false;
    let stringChar = '';

    for (let i = 0; i < innerText.length; i++) {
        const char = innerText[i];
        const prevChar = i > 0 ? innerText[i - 1] : '';

        if ((char === '"' || char === "'") && prevChar !== '\\') {
            if (!inString) {
                inString = true;
                stringChar = char;
            } else if (char === stringChar) {
                inString = false;
            }
        }

        if (!inString) {
            if (char === '(' || char === '[' || char === '{') depth++;
            else if (char === ')' || char === ']' || char === '}') depth--;
            else if (char === ',' && depth === 0) {
                lastCommaPos = i;
                commaCount++;
            }
        }
    }

    const currentSegment = lastCommaPos >= 0 ? innerText.slice(lastCommaPos + 1) : innerText;

    for (let i = 0; i < paramNames.length; i++) {
        if (new RegExp(`\\b${paramNames[i]}\\s*:`).test(currentSegment)) {
            return i;
        }
    }

    return Math.min(commaCount, paramNames.length - 1);
}

function buildSignature(keyword: string, name: string, params: string[]): vscode.SignatureInformation {
    const label = `${keyword} ${name}(${params.join(', ')})`;
    const sig = new vscode.SignatureInformation(label);
    const prefix = `${keyword} ${name}(`;
    let offset = prefix.length;
    sig.parameters = params.map((p, i) => {
        const start = offset;
        const end = start + p.length;
        offset = end + (i < params.length - 1 ? 2 : 0);
        return new vscode.ParameterInformation([start, end] as [number, number]);
    });
    return sig;
}

export const signatureHelpProvider = vscode.languages.registerSignatureHelpProvider(
    'rq',
    {
        provideSignatureHelp(document: vscode.TextDocument, position: vscode.Position): vscode.SignatureHelp | undefined {
            const textBeforeCursor = document.getText(new vscode.Range(
                new vscode.Position(Math.max(0, position.line - 20), 0),
                position
            ));

            const rqMatch = textBeforeCursor.match(/\brq\s+([a-zA-Z_][a-zA-Z0-9_-]*)\s*\(([^;]*)$/s);
            if (rqMatch) {
                const sig = buildSignature('rq', rqMatch[1], ['url', 'headers?', 'body?']);
                const help = new vscode.SignatureHelp();
                help.signatures = [sig];
                help.activeSignature = 0;
                help.activeParameter = getActiveParam(rqMatch[2], RQ_PARAM_NAMES);
                return help;
            }

            const epMatch = textBeforeCursor.match(/\bep\s+([a-zA-Z_][a-zA-Z0-9_-]*)\s*\(([^{;]*)$/s);
            if (epMatch) {
                const sig = buildSignature('ep', epMatch[1], ['url', 'headers?', 'qs?']);
                const help = new vscode.SignatureHelp();
                help.signatures = [sig];
                help.activeSignature = 0;
                help.activeParameter = getActiveParam(epMatch[2], EP_PARAM_NAMES);
                return help;
            }

            const authMatch = textBeforeCursor.match(/\bauth\s+([a-zA-Z_][a-zA-Z0-9_-]*)\s*\(([^{;]*)$/s);
            if (authMatch) {
                const sig = buildSignature('auth', authMatch[1], ['auth_type']);
                const help = new vscode.SignatureHelp();
                help.signatures = [sig];
                help.activeSignature = 0;
                help.activeParameter = getActiveParam(authMatch[2], AUTH_PARAM_NAMES);
                return help;
            }

            return undefined;
        }
    },
    { triggerCharacters: ['(', ','], retriggerCharacters: [',', '\n'] }
);
