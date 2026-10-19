import * as fs from 'fs';
import * as path from 'path';

describe('.vscodeignore', () => {
    const patterns = fs
        .readFileSync(path.join(__dirname, '..', '.vscodeignore'), 'utf8')
        .split(/\r?\n/)
        .map(line => line.trim());

    it.each(['test/**', 'jest.config.js', 'tsconfig.test.json', 'esbuild.js', 'eslint.config.js'])('excludes %s from the packaged extension', pattern => {
        expect(patterns).toContain(pattern);
    });
});
