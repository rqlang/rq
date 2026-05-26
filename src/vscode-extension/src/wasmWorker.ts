import { parentPort } from 'worker_threads';

interface WasmModule {
    [method: string]: (...args: unknown[]) => string | Promise<string>;
}

interface RequestMessage {
    id: number;
    method: string;
    args: unknown[];
}

let wasm: WasmModule | null = null;

function getWasm(): WasmModule {
    if (!wasm) {
        // eslint-disable-next-line @typescript-eslint/no-require-imports
        wasm = require('./wasm/rq_wasm') as WasmModule;
    }
    return wasm;
}

if (!parentPort) {
    throw new Error('wasmWorker must be run as a worker_threads worker');
}

const port = parentPort;

port.on('message', async (message: RequestMessage) => {
    const { id, method, args } = message;
    try {
        const fn = getWasm()[method];
        if (typeof fn !== 'function') {
            throw new Error(`Unknown wasm method: ${method}`);
        }
        const raw = fn(...args);
        const result = raw instanceof Promise ? await raw : raw;
        port.postMessage({ id, result });
    } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        port.postMessage({ id, error: message });
    }
});
