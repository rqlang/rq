import * as path from 'path';
import { Worker } from 'worker_threads';

type WasmMethod =
    | 'list_requests'
    | 'list_auth'
    | 'list_environments'
    | 'list_endpoints'
    | 'list_variables'
    | 'check'
    | 'lint'
    | 'suppression_edit'
    | 'unused_suppression_removal'
    | 'get_request_details'
    | 'get_auth_details'
    | 'get_environment'
    | 'get_endpoint'
    | 'get_variable'
    | 'list_variable_refs'
    | 'list_endpoint_refs'
    | 'version'
    | 'run_request'
    | 'set_debug_logging';

interface PendingCall {
    resolve: (value: string) => void;
    reject: (error: Error) => void;
}

interface WorkerReply {
    id: number;
    result?: string;
    error?: string;
    logs?: string;
}

interface SyncWasmModule {
    [method: string]: (...args: unknown[]) => string | Promise<string>;
}

let worker: Worker | undefined;
let nextId = 1;
const pending = new Map<number, PendingCall>();
let syncWasm: SyncWasmModule | null = null;

export type WasmTransport = 'worker' | 'direct';

let transport: WasmTransport | undefined;

let logListener: ((logs: string) => void) | undefined;

export function onWasmLogs(listener: (logs: string) => void): void {
    logListener = listener;
}

function forwardLogs(logs: string | undefined): void {
    if (logs) { logListener?.(logs); }
}

function takeDirectLogs(): void {
    const take = getSyncWasm()['take_debug_logs'];
    if (typeof take === 'function') {
        forwardLogs(String(take()));
    }
}

export function setWasmTransport(mode: WasmTransport): void {
    transport = mode;
}

function isJestEnvironment(): boolean {
    return typeof process.env.JEST_WORKER_ID !== 'undefined';
}

function activeTransport(): WasmTransport {
    if (transport) { return transport; }
    return isJestEnvironment() ? 'direct' : 'worker';
}

function getSyncWasm(): SyncWasmModule {
    if (!syncWasm) {
        // eslint-disable-next-line @typescript-eslint/no-require-imports
        syncWasm = require('./wasm/rq_wasm') as SyncWasmModule;
    }
    return syncWasm;
}

function getWorker(): Worker {
    if (worker) { return worker; }
    const workerPath = path.join(__dirname, 'wasmWorker.js');
    worker = new Worker(workerPath);
    worker.on('message', (reply: WorkerReply) => {
        forwardLogs(reply.logs);
        const call = pending.get(reply.id);
        if (!call) { return; }
        pending.delete(reply.id);
        if (reply.error !== undefined) {
            call.reject(new Error(reply.error));
        } else {
            call.resolve(reply.result ?? '');
        }
    });
    worker.on('error', (err: unknown) => {
        const normalized = err instanceof Error ? err : new Error(String(err));
        for (const call of pending.values()) { call.reject(normalized); }
        pending.clear();
        worker = undefined;
    });
    worker.on('exit', (code: number) => {
        if (pending.size > 0) {
            const err = new Error(`wasm worker exited with code ${code}`);
            for (const call of pending.values()) { call.reject(err); }
            pending.clear();
        }
        worker = undefined;
    });
    return worker;
}

export function wasmCall(method: WasmMethod, args: unknown[]): Promise<string> {
    if (activeTransport() === 'direct') {
        try {
            return Promise.resolve(getSyncWasm()[method](...args));
        } catch (err) {
            return Promise.reject(err instanceof Error ? err : new Error(String(err)));
        } finally {
            takeDirectLogs();
        }
    }

    return new Promise<string>((resolve, reject) => {
        const id = nextId++;
        pending.set(id, { resolve, reject });
        try {
            getWorker().postMessage({ id, method, args });
        } catch (err) {
            pending.delete(id);
            reject(err instanceof Error ? err : new Error(String(err)));
        }
    });
}

export function initWasmHost(): void {
    if (activeTransport() === 'direct') { return; }
    getWorker();
}

export async function disposeWasmHost(): Promise<void> {
    if (!worker) { return; }
    const w = worker;
    worker = undefined;
    for (const call of pending.values()) {
        call.reject(new Error('wasm worker terminated'));
    }
    pending.clear();
    await w.terminate();
}
