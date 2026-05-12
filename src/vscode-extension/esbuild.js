const esbuild = require('esbuild');

const isWatch = process.argv.includes('--watch');
const isProduction = process.argv.includes('--production');

(async () => {
    const ctx = await esbuild.context({
        entryPoints: ['src/extension.ts'],
        bundle: true,
        outfile: 'out/extension.js',
        external: ['vscode', './wasm/rq_wasm'],
        format: 'cjs',
        platform: 'node',
        target: 'node18',
        sourcemap: !isProduction,
        minify: isProduction,
    });

    if (isWatch) {
        await ctx.watch();
        console.log('Watching for changes...');
    } else {
        await ctx.rebuild();
        await ctx.dispose();
    }
})();
