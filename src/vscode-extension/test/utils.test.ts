import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import { normalizePath, collectMirroredFiles, mirrorToTemp } from '../src/utils';

describe('utils', () => {
    describe('normalizePath', () => {
        it('should return the path as is if no normalization is needed', () => {
            const input = '/home/user/file.rq';
            expect(normalizePath(input)).toBe(input);
        });

        it('should capitalize drive letter on Windows paths', () => {
            const input = 'c:\\Users\\test\\file.rq';
            const expected = 'C:\\Users\\test\\file.rq';
            expect(normalizePath(input)).toBe(expected);
        });

        it('should handle already capitalized drive letters correctly', () => {
            const input = 'D:\\Projects\\rq';
            expect(normalizePath(input)).toBe(input);
        });

        it('should handle paths without drive letters (relative or unix)', () => {
            const input = 'src/test.ts';
            expect(normalizePath(input)).toBe(input);
        });
    });

    describe('mirrorToTemp', () => {
        let workspace: string;
        const tempRoots: string[] = [];

        beforeEach(() => {
            workspace = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'rq-utils-')));
            fs.mkdirSync(path.join(workspace, 'inma'), { recursive: true });
            fs.writeFileSync(path.join(workspace, '_shared.rq'), 'let base_url = "https://example.com";\n');
            fs.writeFileSync(path.join(workspace, '.env'), 'API_KEY=secret-123\n');
            fs.writeFileSync(path.join(workspace, 'inma', 'collections.rq'), 'import "../_shared";\n');
            fs.writeFileSync(path.join(workspace, 'notes.md'), '# not mirrored\n');
        });

        afterEach(() => {
            fs.rmSync(workspace, { recursive: true, force: true });
            while (tempRoots.length > 0) {
                fs.rmSync(tempRoots.pop()!, { recursive: true, force: true });
            }
        });

        function mirror(overrides: Map<string, string> = new Map()): string {
            const target = mirrorToTemp(workspace, overrides);
            tempRoots.push(target);
            return target;
        }

        it('copies the .env file so secrets resolve against the mirror', () => {
            const target = mirror();
            expect(fs.readFileSync(path.join(target, '.env'), 'utf8')).toBe('API_KEY=secret-123\n');
        });

        it('copies rq files from nested directories', () => {
            const target = mirror();
            expect(fs.existsSync(path.join(target, 'inma', 'collections.rq'))).toBe(true);
            expect(fs.existsSync(path.join(target, '_shared.rq'))).toBe(true);
        });

        it('leaves unrelated files out of the mirror', () => {
            const target = mirror();
            expect(fs.existsSync(path.join(target, 'notes.md'))).toBe(false);
        });

        it('writes the override content instead of the file on disk', () => {
            const overrides = new Map([
                [normalizePath(path.join(workspace, 'inma', 'collections.rq')), 'rq draft("http://localhost");\n']
            ]);
            const target = mirror(overrides);
            expect(fs.readFileSync(path.join(target, 'inma', 'collections.rq'), 'utf8'))
                .toBe('rq draft("http://localhost");\n');
        });
    });

    describe('collectMirroredFiles', () => {
        it('returns rq and .env files only', () => {
            const workspace = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'rq-utils-')));
            try {
                fs.writeFileSync(path.join(workspace, 'a.rq'), '');
                fs.writeFileSync(path.join(workspace, '.env'), '');
                fs.writeFileSync(path.join(workspace, '.env.local'), '');
                fs.writeFileSync(path.join(workspace, 'data.json'), '');

                const target = collectMirroredFiles(workspace).map(f => path.basename(f)).sort();

                expect(target).toEqual(['.env', 'a.rq']);
            } finally {
                fs.rmSync(workspace, { recursive: true, force: true });
            }
        });
    });
});
