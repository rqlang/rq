import { normalizePath } from '../src/utils';

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
});
