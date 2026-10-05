type Handler = (payload: unknown) => void;

class FakeWorker {
    static instances: FakeWorker[] = [];
    posted: { id: number; method: string; args: unknown[] }[] = [];
    handlers: Record<string, Handler> = {};

    constructor() {
        FakeWorker.instances.push(this);
    }

    on(event: string, handler: Handler): this {
        this.handlers[event] = handler;
        return this;
    }

    postMessage(message: { id: number; method: string; args: unknown[] }): void {
        this.posted.push(message);
    }

    terminate(): Promise<number> {
        return Promise.resolve(0);
    }
}

jest.mock('worker_threads', () => ({ Worker: FakeWorker }));

import { setWasmTransport, wasmCall } from '../src/wasmHost';

describe('wasm worker restarts', () => {
    it('re-applies debug logging to a replacement worker', async () => {
        setWasmTransport('worker');
        const enabling = wasmCall('set_debug_logging', [true]);
        const first = FakeWorker.instances[0];
        first.handlers.message({ id: first.posted[0].id, result: '' });
        await enabling;
        first.handlers.exit(1);

        wasmCall('lint', ['{}', 'x', 'users.rq']).catch(() => undefined);

        const replacement = FakeWorker.instances[1];
        expect(replacement.posted.map(message => message.method)).toEqual(['set_debug_logging', 'lint']);
        expect(replacement.posted[0].args).toEqual([true]);
    });
});
