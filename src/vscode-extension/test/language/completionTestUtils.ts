import * as vscode from 'vscode';

export function makeDocument(lines: string[], uri: any = { fsPath: '/workspace/current.rq' }) {
    const fullText = lines.join('\n');
    const lineOffsets: number[] = [];
    let off = 0;
    for (const line of lines) {
        lineOffsets.push(off);
        off += line.length + 1;
    }
    const getText = (range?: any): string => {
        if (!range) { return fullText; }
        const startLine = range.start?.line ?? 0;
        const startChar = range.start?.character ?? 0;
        const endLine = range.end?.line ?? lines.length - 1;
        const endChar = range.end?.character ?? (lines[lines.length - 1]?.length ?? 0);
        const startOffset = (lineOffsets[startLine] ?? 0) + startChar;
        const endOffset = (lineOffsets[endLine] ?? 0) + endChar;
        return fullText.slice(startOffset, endOffset);
    };
    return {
        uri,
        lineCount: lines.length,
        lineAt: (i: number | vscode.Position) => {
            const idx = typeof i === 'number' ? i : (i as vscode.Position).line;
            return { text: lines[idx] };
        },
        getText: jest.fn().mockImplementation(getText),
        getWordRangeAtPosition: jest.fn()
    };
}
