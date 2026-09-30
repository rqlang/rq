import * as vscode from 'vscode';

export function formatRqDocument(text: string, tabSize: number): string {
    const { text: protectedText, literals } = protectMultilineStrings(text);
    return restoreMultilineStrings(formatProtectedDocument(protectedText, tabSize), literals);
}

function formatProtectedDocument(text: string, tabSize: number): string {
    const indent = ' '.repeat(tabSize);
    const rawLines: string[] = [];
    for (const raw of normalizeMultilineArrayLiterals(normalizeMultilineCalls(text.split('\n'))))
        {for (const part of splitOnSemicolons(raw)) {rawLines.push(...splitOnBraces(part));}}
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
                for (let i = 0; i < blanks; i++) {output.push('');}
            }
            blankCount = 0;
            output.push(indent.repeat(depth) + trimmed);
            prevTrimmed = trimmed;
            if (trimmed.includes('*/')) {inBlockComment = false;}
            continue;
        }

        if (opensUnterminatedBlockComment(trimmed)) {inBlockComment = true;}

        if (trimmed.startsWith('}') || trimmed.startsWith(']') || trimmed.startsWith(')')) {depth = Math.max(0, depth - 1);}

        if (output.length > 0) {
            const blanks = computeBlanks(blankCount, depth, prevTrimmed, trimmed);
            for (let i = 0; i < blanks; i++) {output.push('');}
        }
        blankCount = 0;

        output.push(depth > 0 ? indent.repeat(depth) + fixSpacing(trimmed) : fixSpacing(trimmed));
        prevTrimmed = trimmed;

        if (isBlockOpener(trimmed)) {depth++;}
    }

    while (output.length > 0 && output[output.length - 1].trim() === '') {output.pop();}
    return output.join('\n') + '\n';
}

function joinArrayClosers(lines: string[]): string[] {
    const result: string[] = [];
    const openersWithParen: boolean[] = [];
    let i = 0;
    while (i < lines.length) {
        const trimmed = lines[i].trim();
        let openerHasParen = true;
        if (/^[\]}]/.test(trimmed)) {openerHasParen = openersWithParen.pop() ?? true;}
        else if (/[[{]$/.test(trimmed)) {openersWithParen.push(parenDepthDelta(trimmed) > 0);}
        if (trimmed === ']' || trimmed === '}') {
            let j = i + 1;
            while (j < lines.length && lines[j].trim() === '') {j++;}
            const closers = openerHasParen ? /^[);]/ : /^;/;
            if (j < lines.length && closers.test(lines[j].trim())) {
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

function parenDepthDelta(line: string): number {
    let depth = 0;
    let stringChar: string | null = null;
    for (let i = 0; i < line.length; i++) {
        const ch = line[i];
        if (stringChar !== null) {
            if (ch === '\\') { i++; continue; }
            if (ch === stringChar) { stringChar = null; }
            continue;
        }
        if (ch === '"' || ch === "'") { stringChar = ch; continue; }
        if (ch === '(') { depth++; }
        else if (ch === ')') { depth--; }
    }
    return depth;
}

function splitTopLevelCommas(content: string): string[] {
    const parts: string[] = [];
    let current = '';
    let stringChar: string | null = null;
    let depth = 0;
    for (let i = 0; i < content.length; i++) {
        const ch = content[i];
        if (stringChar !== null) {
            current += ch;
            if (ch === '\\') { i++; if (i < content.length) { current += content[i]; } continue; }
            if (ch === stringChar) { stringChar = null; }
            continue;
        }
        if (ch === '"' || ch === "'") { stringChar = ch; current += ch; continue; }
        if (ch === '(' || ch === '[' || ch === '{') { depth++; }
        if (ch === ')' || ch === ']' || ch === '}') { depth--; }
        if (ch === ',' && depth === 0) {
            parts.push(current.trim() + ',');
            current = '';
        } else {
            current += ch;
        }
    }
    const last = current.trim();
    if (last) { parts.push(last); }
    return parts;
}

function topLevelContainerBounds(text: string): { open: number; close: number } | null {
    let stringChar: string | null = null;
    let depth = 0;
    let open = -1;
    for (let i = 0; i < text.length; i++) {
        const ch = text[i];
        if (stringChar !== null) {
            if (ch === '\\') { i++; continue; }
            if (ch === stringChar) { stringChar = null; }
            continue;
        }
        if (ch === '"' || ch === "'") { stringChar = ch; continue; }
        if ((ch === '[' || ch === '{') && depth === 0 && open === -1) { open = i; }
        if (ch === '(' || ch === '[' || ch === '{') { depth++; }
        else if (ch === ')' || ch === ']' || ch === '}') {
            depth--;
            if (depth === 0 && open !== -1) { return { open, close: i }; }
        }
    }
    return null;
}

function layoutArgument(text: string): string[] {
    const collapsed = text.replace(/\n/g, ' ');
    const bounds = topLevelContainerBounds(text);
    if (bounds === null || !text.slice(bounds.open, bounds.close).includes('\n')) { return [collapsed]; }
    const entries = splitTopLevelCommas(text.slice(bounds.open + 1, bounds.close));
    if (entries.length === 0) { return [collapsed]; }
    const head = text.slice(0, bounds.open + 1).replace(/\n/g, ' ').trimEnd();
    const tail = text.slice(bounds.close).replace(/\n/g, ' ').trim();
    return [head, ...entries.flatMap(layoutArgument), tail];
}

function reformatMultilineCall(lines: string[]): string[] {
    const joined = lines.map(l => l.trim()).join('\n');
    let stringChar: string | null = null;
    let depth = 0;
    let openIdx = -1;
    let closeIdx = -1;
    for (let i = 0; i < joined.length; i++) {
        const ch = joined[i];
        if (stringChar !== null) {
            if (ch === '\\') { i++; continue; }
            if (ch === stringChar) { stringChar = null; }
            continue;
        }
        if (ch === '"' || ch === "'") { stringChar = ch; continue; }
        if (ch === '(' && depth === 0) { openIdx = i; depth++; }
        else if (ch === '(') { depth++; }
        else if (ch === ')') { depth--; if (depth === 0) { closeIdx = i; break; } }
    }
    if (openIdx < 0 || closeIdx < 0) { return lines; }
    const prefix = joined.slice(0, openIdx + 1).replace(/\n/g, ' ');
    const argsContent = joined.slice(openIdx + 1, closeIdx);
    const suffix = joined.slice(closeIdx + 1).replace(/\n/g, ' ').trim();
    const args = splitTopLevelCommas(argsContent).flatMap(layoutArgument);
    if (args.length === 0) { return lines; }
    return [prefix, ...args, ')' + suffix];
}

function hasUnterminatedString(line: string): boolean {
    let stringChar: string | null = null;
    for (let i = 0; i < line.length; i++) {
        const ch = line[i];
        if (stringChar !== null) {
            if (ch === '\\') { i++; continue; }
            if (ch === stringChar) { stringChar = null; }
        } else if (ch === '"' || ch === "'") {
            stringChar = ch;
        }
    }
    return stringChar !== null;
}

function normalizeMultilineCalls(lines: string[]): string[] {
    const result: string[] = [];
    let i = 0;
    while (i < lines.length) {
        const delta = parenDepthDelta(lines[i]);
        const tail = lines[i].trimEnd();
        const isParamContinuation = tail.endsWith(',') || tail.endsWith('(');
        if (delta > 0 && isParamContinuation) {
            const block = [lines[i]];
            let depth = delta;
            i++;
            while (i < lines.length && depth > 0) {
                depth += parenDepthDelta(lines[i]);
                block.push(lines[i]);
                i++;
            }
            const canReformat = block.length > 1 && !block.some(hasUnterminatedString);
            result.push(...(canReformat ? reformatMultilineCall(block) : block));
        } else {
            result.push(lines[i]);
            i++;
        }
    }
    return result;
}

function dollarBracketIndex(line: string): number {
    let inString = false;
    for (let i = 0; i < line.length - 1; i++) {
        if (inString && line[i] === '\\') { i++; continue; }
        if (line[i] === '"') { inString = !inString; continue; }
        if (!inString && line[i] === '$' && line[i + 1] === '[') { return i; }
    }
    return -1;
}

function hasCloserOutsideString(text: string): boolean {
    let inString = false;
    for (let i = 0; i < text.length; i++) {
        if (inString && text[i] === '\\') { i++; continue; }
        if (text[i] === '"') { inString = !inString; continue; }
        if (!inString && text[i] === ']') { return true; }
    }
    return false;
}

function normalizeMultilineArrayLiterals(lines: string[]): string[] {
    const result: string[] = [];
    let i = 0;
    while (i < lines.length) {
        const openerIdx = dollarBracketIndex(lines[i]);
        if (openerIdx !== -1) {
            const afterOpener = lines[i].slice(openerIdx + 2);
            if (afterOpener.trim().length > 0 && !hasUnterminatedString(afterOpener) && !hasCloserOutsideString(afterOpener)) {
                const prefix = lines[i].slice(0, openerIdx + 2);
                const collected = [afterOpener.trim()];
                i++;
                while (i < lines.length && !lines[i].trimStart().startsWith(']')) {
                    if (lines[i].trim()) { collected.push(lines[i].trim()); }
                    i++;
                }
                result.push(prefix);
                for (const entry of splitTopLevelCommas(collected.join(' '))) { result.push(entry.trim()); }
                continue;
            }
        }
        result.push(lines[i]);
        i++;
    }
    return result;
}

function splitLineOnCommas(line: string): string[] {
    const parts: string[] = [];
    let current = '';
    let stringChar: string | null = null;
    let depth = 0;
    for (let i = 0; i < line.length; i++) {
        const ch = line[i];
        if (stringChar !== null) {
            current += ch;
            if (ch === '\\') { i++; if (i < line.length) { current += line[i]; } continue; }
            if (ch === stringChar) { stringChar = null; }
            continue;
        }
        if (ch === '"' || ch === "'") { stringChar = ch; current += ch; continue; }
        if (ch === '(' || ch === '[' || ch === '{') {depth++;}
        if (ch === ')' || ch === ']' || ch === '}') {depth--;}
        if (ch === ',' && depth === 0) {
            parts.push(current + ',');
            current = '';
        } else {
            current += ch;
        }
    }
    if (current.trim()) {parts.push(current);}
    return parts.length > 1 ? parts : [line];
}

function splitOnBraces(line: string): string[] {
    const parts: string[] = [];
    let current = '';
    let stringChar: string | null = null;
    let jsonDepth = 0;
    let bracketDepth = 0;
    for (let i = 0; i < line.length; i++) {
        const ch = line[i];
        if (stringChar !== null) {
            current += ch;
            if (ch === '\\') { i++; if (i < line.length) { current += line[i]; } continue; }
            if (ch === stringChar) { stringChar = null; }
            continue;
        }
        if (ch === '"' || ch === "'") { stringChar = ch; current += ch; continue; }
        if (ch === '[') {
            bracketDepth++;
            current += ch;
            continue;
        }
        if (ch === ']') {
            if (bracketDepth > 0) { bracketDepth--; current += ch; continue; }
            if (current.trim()) {parts.push(current);}
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
            if (current.trim()) {parts.push(current);}
            let closer = '}';
            if (i + 1 < line.length && (line[i + 1] === ';' || line[i + 1] === ',')) { closer += line[i + 1]; i++; }
            parts.push(closer);
            current = '';
            continue;
        }
        current += ch;
    }
    if (current.trim()) {parts.push(current);}
    return parts.length > 0 ? parts : [line];
}

function splitOnSemicolons(line: string): string[] {
    const result: string[] = [];
    let current = '';
    let stringChar: string | null = null;
    for (let i = 0; i < line.length; i++) {
        const ch = line[i];
        if (stringChar !== null) {
            current += ch;
            if (ch === '\\') { i++; if (i < line.length) { current += line[i]; } continue; }
            if (ch === stringChar) { stringChar = null; }
            continue;
        }
        if (ch === '"' || ch === "'") { stringChar = ch; current += ch; continue; }
        current += ch;
        if (ch === ';') {
            result.push(current);
            current = '';
        }
    }
    if (current.trim()) {result.push(current);}
    return result.length > 0 ? result : [line];
}

function computeBlanks(blankCount: number, depth: number, prev: string, curr: string): number {
    if (curr.startsWith('}') || curr.startsWith(']') || curr.startsWith(')')) {return 0;}
    if (isStickyPair(prev, curr)) {return 0;}
    if (depth === 0 && needsBlankSeparator(prev, curr)) {return 1;}
    return Math.min(blankCount, 1);
}

function needsBlankSeparator(prev: string, curr: string): boolean {
    if (prev.startsWith('}')) {return true;}
    if (/^(ep|env|auth)\b/.test(curr)) {return true;}
    if (/^(ep|env|auth)\b/.test(prev)) {return true;}
    return false;
}

function isStickyPair(prev: string, curr: string): boolean {
    if (/^\[(?:method|auth|timeout)\s*\(/.test(prev)) {return true;}
    if (prev.startsWith('//') || prev.startsWith('/*')) {return true;}
    if (prev.startsWith('import ') && curr.startsWith('import ')) {return true;}
    return false;
}

function isBlockOpener(trimmed: string): boolean {
    return trimmed.endsWith('{') || trimmed.endsWith('[') || trimmed.endsWith('(');
}

function fixSpacing(trimmed: string): string {
    const commentStart = commentStartIndex(trimmed);
    if (commentStart !== -1) {
        return fixSpacing(trimmed.slice(0, commentStart)) + trimmed.slice(commentStart);
    }

    let result = trimmed
        .replace(/\b(import|let|rq|ep|auth|env)\s{2,}/g, '$1 ')
        .replace(/^(let\s+\w+)\s*=\s*/, '$1 = ')
        .replace(/\b(rq|ep|auth)\s+([^\s(]+)\s*\(/g, '$1 $2(')
        .replace(/\s+;/g, ';')
        .replace(/([^$\s])\s*\{$/g, '$1 {')
        .replace(/"([^"]*)"(\s*):(\s*)"/g, '"$1": "');

    result = result.replace(/([a-zA-Z_][a-zA-Z0-9_]*)\s*:\s*(?=[^\s/])/g, (match, name, offset) => {
        const before = result.slice(0, offset);
        if ((before.match(/"/g) || []).length % 2 !== 0) {return match;}
        return `${name}: `;
    });

    return normalizeCommaSpacing(collapseSpaceRuns(result));
}

function collapseSpaceRuns(s: string): string {
    let out = '';
    let stringChar: string | null = null;
    for (let i = 0; i < s.length; i++) {
        const ch = s[i];
        if (stringChar !== null) {
            out += ch;
            if (ch === '\\') {
                i++;
                if (i < s.length) { out += s[i]; }
            } else if (ch === stringChar) {
                stringChar = null;
            }
        } else if (ch === '"' || ch === "'") {
            stringChar = ch;
            out += ch;
        } else if (ch === ' ' || ch === '\t') {
            while (i + 1 < s.length && (s[i + 1] === ' ' || s[i + 1] === '\t')) { i++; }
            out += ' ';
        } else {
            out += ch;
        }
    }
    return out;
}

function opensUnterminatedBlockComment(line: string): boolean {
    let stringChar: string | null = null;
    for (let i = 0; i < line.length; i++) {
        const ch = line[i];
        if (stringChar !== null) {
            if (ch === '\\') { i++; continue; }
            if (ch === stringChar) { stringChar = null; }
            continue;
        }
        if (ch === '"' || ch === "'") { stringChar = ch; continue; }
        if (ch === '/' && line[i + 1] === '/') { return false; }
        if (ch === '/' && line[i + 1] === '*') {
            const end = line.indexOf('*/', i + 2);
            if (end === -1) { return true; }
            i = end + 1;
        }
    }
    return false;
}

function commentStartIndex(line: string): number {
    let stringChar: string | null = null;
    for (let i = 0; i < line.length; i++) {
        const ch = line[i];
        if (stringChar !== null) {
            if (ch === '\\') { i++; continue; }
            if (ch === stringChar) { stringChar = null; }
            continue;
        }
        if (ch === '"' || ch === "'") { stringChar = ch; continue; }
        if (ch === '/' && (line[i + 1] === '/' || line[i + 1] === '*')) { return i; }
    }
    return -1;
}

function normalizeCommaSpacing(s: string): string {
    let out = '';
    let stringChar: string | null = null;
    for (let i = 0; i < s.length; i++) {
        const ch = s[i];
        if (stringChar !== null) {
            out += ch;
            if (ch === '\\') {
                i++;
                if (i < s.length) { out += s[i]; }
            } else if (ch === stringChar) {
                stringChar = null;
            }
        } else if (ch === '"' || ch === "'") {
            stringChar = ch;
            out += ch;
        } else if (ch === ',') {
            while (out.length > 0 && out[out.length - 1] === ' ') { out = out.slice(0, -1); }
            out += ',';
            while (i + 1 < s.length && s[i + 1] === ' ') { i++; }
            if (i + 1 < s.length) { out += ' '; }
        } else {
            out += ch;
        }
    }
    return out;
}

const multilineStringMarker = '\u0000';

function protectMultilineStrings(text: string): { text: string; literals: string[] } {
    const literals: string[] = [];
    if (text.includes(multilineStringMarker)) { return { text, literals }; }
    let out = '';
    let i = 0;
    while (i < text.length) {
        const ch = text[i];
        if (ch === '/' && text[i + 1] === '/') {
            const end = text.indexOf('\n', i);
            const stop = end === -1 ? text.length : end;
            out += text.slice(i, stop);
            i = stop;
            continue;
        }
        if (ch === '/' && text[i + 1] === '*') {
            const end = text.indexOf('*/', i + 2);
            const stop = end === -1 ? text.length : end + 2;
            out += text.slice(i, stop);
            i = stop;
            continue;
        }
        if (ch !== '"' && ch !== "'") {
            out += ch;
            i++;
            continue;
        }
        let j = i + 1;
        let closed = false;
        while (j < text.length) {
            if (text[j] === '\\') { j += 2; continue; }
            if (text[j] === ch) { j++; closed = true; break; }
            j++;
        }
        if (!closed) {
            out += ch;
            i++;
            continue;
        }
        const literal = text.slice(i, j);
        if (literal.includes('\n')) {
            out += `${ch}${multilineStringMarker}${literals.length}${multilineStringMarker}${ch}`;
            literals.push(literal);
        } else {
            out += literal;
        }
        i = j;
    }
    return { text: out, literals };
}

function restoreMultilineStrings(text: string, literals: string[]): string {
    if (literals.length === 0) { return text; }
    return text.replace(
        new RegExp(`(["'])${multilineStringMarker}(\\d+)${multilineStringMarker}\\1`, 'g'),
        (match, _quote, index) => literals[Number(index)] ?? match
    );
}

export const formattingProvider = vscode.languages.registerDocumentFormattingEditProvider(
    'rq',
    {
        provideDocumentFormattingEdits(document: vscode.TextDocument, options: vscode.FormattingOptions): vscode.TextEdit[] {
            const text = document.getText();
            const formatted = formatRqDocument(text, options.tabSize);
            if (formatted === text) {return [];}
            const lastLine = document.lineAt(document.lineCount - 1);
            const fullRange = new vscode.Range(
                new vscode.Position(0, 0),
                new vscode.Position(document.lineCount - 1, lastLine.text.length)
            );
            return [vscode.TextEdit.replace(fullRange, formatted)];
        }
    }
);
