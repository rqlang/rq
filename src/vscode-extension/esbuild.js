const esbuild = require('esbuild');

const isWatch = process.argv.includes('--watch');
const isProduction = process.argv.includes('--production');

const sharedOptions = {
    bundle: true,
    external: ['vscode', './wasm/rq_wasm'],
    format: 'cjs',
    platform: 'node',
    target: 'node18',
    sourcemap: !isProduction,
    minify: isProduction,
    loader: { '.md': 'text' },
};

(async () => {
    const extensionCtx = await esbuild.context({
        ...sharedOptions,
        entryPoints: ['src/extension.ts'],
        outfile: 'out/extension.js',
    });

    const mcpServerCtx = await esbuild.context({
        ...sharedOptions,
        entryPoints: ['src/mcp/server.ts'],
        outfile: 'out/mcpServer.js',
    });

    const workerCtx = await esbuild.context({
        ...sharedOptions,
        entryPoints: ['src/wasmWorker.ts'],
        outfile: 'out/wasmWorker.js',
    });

    if (isWatch) {
        await Promise.all([extensionCtx.watch(), workerCtx.watch(), mcpServerCtx.watch()]);
        console.log('Watching for changes...');
    } else {
        await Promise.all([extensionCtx.rebuild(), workerCtx.rebuild(), mcpServerCtx.rebuild()]);
        await Promise.all([extensionCtx.dispose(), workerCtx.dispose(), mcpServerCtx.dispose()]);
    }
})();
