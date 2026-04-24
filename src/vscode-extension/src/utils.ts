import * as vscode from 'vscode';
import * as fs from 'fs';
import * as path from 'path';
import * as os from 'os';

/**
 * Normalize a file path to remove Windows extended path prefix and standardize drive letter.
 */
export function normalizePath(p: string): string {
    const stripped = p.replace(/^\\\\\?\\/, '');
    let normalized = vscode.Uri.file(stripped).fsPath;
    if (/^[a-z]:/.test(normalized)) {
        normalized = normalized.charAt(0).toUpperCase() + normalized.slice(1);
    }
    return normalized;
}

export function collectRqFiles(dir: string): string[] {
    return collectAllFiles(dir).filter(f => f.endsWith('.rq'));
}

export function collectAllFiles(dir: string): string[] {
    const results: string[] = [];
    try {
        for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
            const full = path.join(dir, entry.name);
            if (entry.isDirectory()) {
                results.push(...collectAllFiles(full));
            } else if (entry.isFile()) {
                results.push(full);
            }
        }
    } catch {
        // skip unreadable dirs
    }
    return results;
}

export async function collectAllFilesAsync(dir: string): Promise<string[]> {
    const results: string[] = [];
    try {
        const entries = await fs.promises.readdir(dir, { withFileTypes: true });
        await Promise.all(entries.map(async entry => {
            const full = path.join(dir, entry.name);
            if (entry.isDirectory()) {
                results.push(...await collectAllFilesAsync(full));
            } else if (entry.isFile()) {
                results.push(full);
            }
        }));
    } catch {
        // skip unreadable dirs
    }
    return results;
}

export function mirrorToTemp(folderPath: string, overrides: Map<string, string>): string {
    const tempDir = normalizePath(fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'rq-check-'))));
    try {
        for (const filePath of collectRqFiles(folderPath)) {
            const relative = path.relative(folderPath, filePath);
            const dest = path.join(tempDir, relative);
            fs.mkdirSync(path.dirname(dest), { recursive: true });
            const override = overrides.get(normalizePath(filePath));
            fs.writeFileSync(dest, override !== undefined ? override : fs.readFileSync(filePath));
        }
    } catch (e) {
        try { fs.rmSync(tempDir, { recursive: true, force: true }); } catch { /* ignore */ }
        throw e;
    }
    return tempDir;
}

export function applyTreeItemLoading(
    item: vscode.TreeItem,
    loading: boolean,
    originalIcons: WeakMap<vscode.TreeItem, vscode.TreeItem['iconPath']>,
    fireChange: (item: vscode.TreeItem) => void
): void {
    if (loading) {
        if (!originalIcons.has(item)) {
            originalIcons.set(item, item.iconPath);
        }
        item.iconPath = new vscode.ThemeIcon('sync~spin');
    } else {
        item.iconPath = originalIcons.get(item) ?? item.iconPath;
        originalIcons.delete(item);
    }
    fireChange(item);
}
