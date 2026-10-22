const esbuild = require('esbuild');
const fs = require('fs');
const path = require('path');

const isProduction = process.argv.includes('--production');
const distDir = path.join(__dirname, 'dist');
const wasmSourceDir = path.join(__dirname, '..', 'rq-wasm', 'src', 'wasm');
const wasmFiles = ['rq_wasm.js', 'rq_wasm_bg.wasm'];
const { version, devDependencies } = require('./package.json');
const bundledPackages = Object.keys(devDependencies).filter(name => name !== 'esbuild');

const ownDependencies = {
    name: 'own-dependencies',
    setup(build) {
        build.onResolve({ filter: /^[^./]/ }, args => {
            const owned = bundledPackages.some(name => args.path === name || args.path.startsWith(`${name}/`));
            if (!owned || args.resolveDir === __dirname) { return undefined; }
            return build.resolve(args.path, { kind: args.kind, resolveDir: __dirname });
        });
    },
};

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
    const result = await esbuild.build({
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
        plugins: [ownDependencies],
        metafile: true,
        banner: { js: '#!/usr/bin/env node' },
        define: { 'process.env.RQ_MCP_VERSION': JSON.stringify(version) },
    });
    const foreign = Object.keys(result.metafile.inputs).filter(input => input.includes('node_modules') && !path.resolve(input).startsWith(path.join(__dirname, 'node_modules')));
    if (foreign.length > 0) {
        throw new Error(`bundled dependencies from outside src/rq-mcp/node_modules: ${foreign.slice(0, 5).join(', ')}`);
    }
    copyWasm();
    fs.copyFileSync(path.join(__dirname, '..', '..', 'LICENSE'), path.join(distDir, 'LICENSE'));
    fs.chmodSync(path.join(distDir, 'server.js'), 0o755);
})().catch(err => {
    console.error(err.message);
    process.exit(1);
});
