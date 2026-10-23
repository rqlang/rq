import { draftsJson, isInsideFolder, normalizePath } from '../src/utils';

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

    describe('isInsideFolder', () => {
        it('accepts a file under the folder', () => {
            expect(isInsideFolder('/repo/api/a.rq', '/repo')).toBe(true);
        });

        it('accepts the folder itself and a folder given with a trailing slash', () => {
            expect(isInsideFolder('/repo', '/repo')).toBe(true);
            expect(isInsideFolder('/repo/a.rq', '/repo/')).toBe(true);
        });

        it('rejects a sibling folder that merely shares the prefix', () => {
            expect(isInsideFolder('/repo-other/a.rq', '/repo')).toBe(false);
        });

        it('compares Windows paths whatever their separators and drive case', () => {
            expect(isInsideFolder('c:\\repo\\a.rq', 'C:/repo')).toBe(true);
        });
    });

    describe('draftsJson', () => {
        it('keys every draft by its path with forward slashes', () => {
            const target = JSON.parse(draftsJson([{ path: 'C:\\repo\\api\\users.rq', source: 'rq a("http://x");\n' }]));
            expect(target).toEqual({ 'C:/repo/api/users.rq': 'rq a("http://x");\n' });
        });

        it('is an empty map when there are no drafts', () => {
            expect(draftsJson()).toBe('{}');
        });
    });
});
