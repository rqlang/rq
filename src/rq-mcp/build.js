const esbuild = require('esbuild');
const fs = require('fs');
const path = require('path');

const isProduction = process.argv.includes('--production');
const distDir = path.join(__dirname, 'dist');
const wasmSourceDir = path.join(__dirname, '..', 'rq-wasm', 'src', 'wasm');
const wasmFiles = ['rq_wasm.js', 'rq_wasm_bg.wasm'];
const { version } = require('./package.json');

function copyWasm() {
    const missing = wasmFiles.filter(file => !fs.existsSync(path.join(wasmSourceDir, file)));
    if (missing.length > 0) {
        throw new Error(`rq-wasm is not built (missing ${missing.join(', ')}); run npm run build-wasm first`);
    }
    fs.mkdirSync(path.join(distDir, 'wasm'), { recursive: true });
    for (const file of wasmFiles) {
        fs.copyFileSync(path.join(wasmSourceDir, file), path.join(distDir, 'wasm', file));
    }
}

(async () => {
    fs.rmSync(distDir, { recursive: true, force: true });
    await esbuild.build({
        entryPoints: [path.join(__dirname, '..', 'vscode-extension', 'src', 'mcp', 'server.ts')],
        outfile: path.join(distDir, 'server.js'),
        bundle: true,
        external: ['./wasm/rq_wasm'],
        format: 'cjs',
        platform: 'node',
        target: 'node18',
        minify: isProduction,
        sourcemap: !isProduction,
        loader: { '.md': 'text' },
        nodePaths: [path.join(__dirname, 'node_modules')],
        banner: { js: '#!/usr/bin/env node' },
        define: { 'process.env.RQ_MCP_VERSION': JSON.stringify(version) },
    });
    copyWasm();
    fs.copyFileSync(path.join(__dirname, '..', '..', 'LICENSE'), path.join(distDir, 'LICENSE'));
    fs.chmodSync(path.join(distDir, 'server.js'), 0o755);
})().catch(err => {
    console.error(err.message);
    process.exit(1);
});
