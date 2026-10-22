import * as fs from 'fs';
import * as path from 'path';

/**
 * Normalize a file path to remove Windows extended path prefix and standardize drive letter.
 */
export function normalizePath(p: string): string {
    const stripped = p.replace(/^\\\\\?\\/, '');
    let normalized = path.normalize(stripped);
    if (/^[a-z]:/.test(normalized)) {
        normalized = normalized.charAt(0).toUpperCase() + normalized.slice(1);
    }
    return normalized;
}

export async function isDirectory(target: string): Promise<boolean> {
    try {
        return (await fs.promises.stat(target)).isDirectory();
    } catch {
        return false;
    }
}

export async function directoryOf(source: string): Promise<string> {
    try {
        const stat = await fs.promises.stat(source);
        return stat.isDirectory() ? source : path.dirname(source);
    } catch {
        return source;
    }
}

export interface DraftFile {
    path: string;
    source: string;
}

export function draftsJson(drafts: DraftFile[] = []): string {
    return JSON.stringify(Object.fromEntries(drafts.map(draft => [draft.path.replace(/\\/g, '/'), draft.source])));
}

export async function buildSecretsMap(source: string): Promise<string> {
    const dir = await directoryOf(source);

    let envFile: string | null = null;
    try {
        envFile = await fs.promises.readFile(path.join(dir, '.env'), 'utf8');
    } catch {
        envFile = null;
    }

    const osVars: [string, string][] = [];
    for (const [key, value] of Object.entries(process.env)) {
        if (key.startsWith('RQ__') && value !== undefined) {
            osVars.push([key, value]);
        }
    }
    return JSON.stringify({ env_file: envFile, os_vars: osVars });
}

export const CANCELLED_MESSAGE = 'Cancelled by user';
