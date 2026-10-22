import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';

const serverEntry = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', 'dist', 'server.js');

export function tempWorkspace(files = {}) {
    const dir = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'rq-mcp-')));
    for (const [name, source] of Object.entries(files)) {
        fs.mkdirSync(path.dirname(path.join(dir, name)), { recursive: true });
        fs.writeFileSync(path.join(dir, name), source);
    }
    return dir;
}

export async function connect(cwd = tempWorkspace()) {
    const client = new Client({ name: 'rq-mcp-test', version: '0.0.0' });
    await client.connect(new StdioClientTransport({ command: process.execPath, args: [serverEntry], cwd, stderr: 'ignore' }));
    return client;
}

export async function callJson(client, name, args) {
    const result = await client.callTool({ name, arguments: args });
    return JSON.parse(result.content[0].text);
}
