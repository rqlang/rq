import * as fs from 'fs';
import * as path from 'path';
import * as os from 'os';

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

const SKIPPED_DIRECTORIES = new Set(['node_modules', '.git', 'target']);

export function collectRqFiles(dir: string): string[] {
    return collectAllFiles(dir).filter(f => f.endsWith('.rq'));
}

export function collectAllFiles(dir: string): string[] {
    const results: string[] = [];
    try {
        for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
            if (entry.isDirectory() && SKIPPED_DIRECTORIES.has(entry.name)) {
                continue;
            }
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
        for (const entry of entries) {
            if (entry.isDirectory() && SKIPPED_DIRECTORIES.has(entry.name)) {
                continue;
            }
            const full = path.join(dir, entry.name);
            if (entry.isDirectory()) {
                results.push(...await collectAllFilesAsync(full));
            } else if (entry.isFile()) {
                results.push(full);
            }
        }
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

export async function buildFilesMap(source: string, drafts: DraftFile[] = []): Promise<string> {
    const dir = await directoryOf(source);
    const files: Record<string, string> = {};
    for (const filePath of await collectAllFilesAsync(dir)) {
        const normalized = filePath.replace(/\\/g, '/');
        try {
            files[normalized] = await fs.promises.readFile(filePath, 'utf8');
        } catch {
            continue;
        }
    }
    for (const draft of drafts) {
        files[draft.path.replace(/\\/g, '/')] = draft.source;
    }
    return JSON.stringify(files);
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
