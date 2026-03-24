import * as vscode from 'vscode';

export function formatRqDocument(text: string, tabSize: number): string {
    const indent = ' '.repeat(tabSize);
    const rawLines: string[] = [];
    for (const raw of text.split('\n'))
        for (const part of splitOnSemicolons(raw)) rawLines.push(...splitOnBraces(part));
    const lines = joinArrayClosers(splitArrayEntries(rawLines));
    const output: string[] = [];
    let depth = 0;
    let inBlockComment = false;
    let blankCount = 0;
    let prevTrimmed = '';

    for (const raw of lines) {
        const trimmed = raw.trim();

        if (trimmed === '') {
            blankCount++;
            continue;
        }

        if (trimmed === '{') {
            if (output.length > 0) {
                output[output.length - 1] = output[output.length - 1].trimEnd() + ' {';
            } else {
                output.push('{');
            }
            blankCount = 0;
            depth++;
            prevTrimmed = trimmed;
            continue;
        }

        if (inBlockComment) {
            if (output.length > 0) {
                const blanks = computeBlanks(blankCount, depth, prevTrimmed, trimmed);
                for (let i = 0; i < blanks; i++) output.push('');
            }
            blankCount = 0;
            output.push(indent.repeat(depth) + trimmed);
            prevTrimmed = trimmed;
            if (trimmed.includes('*/')) inBlockComment = false;
            continue;
        }

        if (trimmed.startsWith('/*') && !trimmed.includes('*/')) inBlockComment = true;

        if (trimmed.startsWith('}') || trimmed.startsWith(']')) depth = Math.max(0, depth - 1);

        if (output.length > 0) {
            const blanks = computeBlanks(blankCount, depth, prevTrimmed, trimmed);
            for (let i = 0; i < blanks; i++) output.push('');
        }
        blankCount = 0;

        output.push(depth > 0 ? indent.repeat(depth) + fixSpacing(trimmed) : fixSpacing(trimmed));
        prevTrimmed = trimmed;

        if (isBlockOpener(trimmed)) depth++;
    }

    while (output.length > 0 && output[output.length - 1].trim() === '') output.pop();
    return output.join('\n') + '\n';
}

function joinArrayClosers(lines: string[]): string[] {
    const result: string[] = [];
    let i = 0;
    while (i < lines.length) {
        const trimmed = lines[i].trim();
        if (trimmed === ']' || trimmed === '}') {
            let j = i + 1;
            while (j < lines.length && lines[j].trim() === '') j++;
            if (j < lines.length && /^[);]/.test(lines[j].trim())) {
                result.push(trimmed + lines[j].trim());
                i = j + 1;
                continue;
            }
        }
        result.push(lines[i]);
        i++;
    }
    return result;
}

function splitArrayEntries(lines: string[]): string[] {
    const result: string[] = [];
    let arrayDepth = 0;
    for (const line of lines) {
        const trimmed = line.trim();
        if (trimmed.endsWith('[') && !/\$\{[^}]*$/.test(trimmed)) {
            arrayDepth++;
            result.push(line);
            continue;
        }
        if (trimmed.startsWith(']')) {
            arrayDepth = Math.max(0, arrayDepth - 1);
            result.push(line);
            continue;
        }
        if (arrayDepth > 0) {
            result.push(...splitLineOnCommas(line));
        } else {
            result.push(line);
        }
    }
    return result;
}

function splitLineOnCommas(line: string): string[] {
    const parts: string[] = [];
    let current = '';
    let inString = false;
    let depth = 0;
    for (const ch of line) {
        if (ch === '"') inString = !inString;
        if (!inString) {
            if (ch === '(' || ch === '[' || ch === '{') depth++;
            if (ch === ')' || ch === ']' || ch === '}') depth--;
        }
        if (ch === ',' && !inString && depth === 0) {
            parts.push(current + ',');
            current = '';
        } else {
            current += ch;
        }
    }
    if (current.trim()) parts.push(current);
    return parts.length > 1 ? parts : [line];
}

function splitOnBraces(line: string): string[] {
    const parts: string[] = [];
    let current = '';
    let inString = false;
    let jsonDepth = 0;
    let bracketDepth = 0;
    for (let i = 0; i < line.length; i++) {
        const ch = line[i];
        if (ch === '"') inString = !inString;
        if (!inString) {
            if (ch === '[') {
                bracketDepth++;
                current += ch;
                continue;
            }
            if (ch === ']') {
                if (bracketDepth > 0) { bracketDepth--; current += ch; continue; }
                if (current.trim()) parts.push(current);
                let closer = ']';
                if (i + 1 < line.length && (line[i + 1] === ';' || line[i + 1] === ',')) { closer += line[i + 1]; i++; }
                parts.push(closer);
                current = '';
                continue;
            }
            if (ch === '{' && i > 0 && line[i - 1] === '$') {
                jsonDepth++;
                current += ch;
                continue;
            }
            if (ch === '{' && jsonDepth === 0) {
                current += ch;
                parts.push(current);
                current = '';
                continue;
            }
            if (ch === '}') {
                if (jsonDepth > 0) { jsonDepth--; current += ch; continue; }
                if (current.trim()) parts.push(current);
                let closer = '}';
                if (i + 1 < line.length && line[i + 1] === ';') { closer = '};'; i++; }
                parts.push(closer);
                current = '';
                continue;
            }
        }
        current += ch;
    }
    if (current.trim()) parts.push(current);
    return parts.length > 0 ? parts : [line];
}

function splitOnSemicolons(line: string): string[] {
    const result: string[] = [];
    let current = '';
    let inString = false;
    for (const ch of line) {
        if (ch === '"') inString = !inString;
        current += ch;
        if (ch === ';' && !inString) {
            result.push(current);
            current = '';
        }
    }
    if (current.trim()) result.push(current);
    return result.length > 0 ? result : [line];
}

function computeBlanks(blankCount: number, depth: number, prev: string, curr: string): number {
    if (curr.startsWith('}') || curr.startsWith(']')) return 0;
    if (isStickyPair(prev, curr)) return 0;
    if (depth === 0 && needsBlankSeparator(prev, curr)) return 1;
    return Math.min(blankCount, 1);
}

function needsBlankSeparator(prev: string, curr: string): boolean {
    if (prev.startsWith('}')) return true;
    if (/^(ep|env|auth)\b/.test(curr)) return true;
    if (/^(ep|env|auth)\b/.test(prev)) return true;
    return false;
}

function isStickyPair(prev: string, curr: string): boolean {
    if (/^\[(?:method|auth|timeout)\s*\(/.test(prev)) return true;
    if (prev.startsWith('//') || prev.startsWith('/*')) return true;
    if (prev.startsWith('import ') && curr.startsWith('import ')) return true;
    return false;
}

function isBlockOpener(trimmed: string): boolean {
    return trimmed.endsWith('{') || trimmed.endsWith('[');
}

function fixSpacing(trimmed: string): string {
    let result = trimmed
        .replace(/\b(import|let|rq|ep|auth|env)\s{2,}/g, '$1 ')
        .replace(/^(let\s+\w+)\s*=\s*/, '$1 = ')
        .replace(/\b(rq|ep|auth)\s+([^\s(]+)\s*\(/g, '$1 $2(')
        .replace(/\s+;/g, ';')
        .replace(/([^$\s])\s*\{$/g, '$1 {')
        .replace(/"([^"]*)"(\s*):(\s*)"/g, '"$1": "');

    result = result.replace(/([a-zA-Z_][a-zA-Z0-9_]*)\s*:\s*(?=[^\s/])/g, (match, name, offset) => {
        const before = result.slice(0, offset);
        if ((before.match(/"/g) || []).length % 2 !== 0) return match;
        return `${name}: `;
    });

    return result.replace(/([")\]])\s*,\s*([^\s,\n])/g, '$1, $2');
}

export const formattingProvider = vscode.languages.registerDocumentFormattingEditProvider(
    'rq',
    {
        provideDocumentFormattingEdits(document: vscode.TextDocument, options: vscode.FormattingOptions): vscode.TextEdit[] {
            const text = document.getText();
            const formatted = formatRqDocument(text, options.tabSize);
            if (formatted === text) return [];
            const lastLine = document.lineAt(document.lineCount - 1);
            const fullRange = new vscode.Range(
                new vscode.Position(0, 0),
                new vscode.Position(document.lineCount - 1, lastLine.text.length)
            );
            return [vscode.TextEdit.replace(fullRange, formatted)];
        }
    }
);
