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

export const AUTH_FIELDS = {
    bearer: [{ name: 'token', required: true }],
    oauth2_authorization_code: [
        { name: 'client_id', required: true },
        { name: 'authorization_url', required: true },
        { name: 'token_url', required: true },
        { name: 'redirect_uri', required: true },
        { name: 'client_secret', required: false },
        { name: 'scope', required: false },
        { name: 'code_challenge_method', required: false },
        { name: 'use_state', required: false },
    ],
    oauth2_client_credentials: [
        { name: 'client_id', required: true },
        { name: 'token_url', required: true },
        { name: 'client_secret', required: false },
        { name: 'scope', required: false },
        { name: 'cert_file', required: false },
        { name: 'cert_password', required: false },
    ],
    oauth2_implicit: [
        { name: 'client_id', required: true },
        { name: 'authorization_url', required: true },
        { name: 'redirect_uri', required: true },
        { name: 'scope', required: false },
    ],
};
