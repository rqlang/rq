import * as vscode from 'vscode';

/**
 * Normalize a file path to remove Windows extended path prefix and standardize drive letter.
 */
export function normalizePath(p: string): string {
    let normalized = vscode.Uri.file(p).fsPath;
    // Normalize drive letter to uppercase for consistency
    if (/^[a-z]:/.test(normalized)) {
        normalized = normalized.charAt(0).toUpperCase() + normalized.slice(1);
    }
    return normalized;
}
