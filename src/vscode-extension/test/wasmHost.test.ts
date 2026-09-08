const mockLint = jest.fn();

jest.mock('../src/wasm/rq_wasm', () => ({ lint: mockLint }), { virtual: true });

import { setWasmTransport, wasmCall } from '../src/wasmHost';

describe('wasm transport selection', () => {
    beforeEach(() => jest.clearAllMocks());

    it('exposes lint through the shared call seam', async () => {
        mockLint.mockReturnValue('{"ok":true,"diagnostics":[]}');

        const target = await wasmCall('lint', ['{}', 'rq list("http://x");', 'users.rq']);

        expect(JSON.parse(target).ok).toBe(true);
        expect(mockLint).toHaveBeenCalledWith('{}', 'rq list("http://x");', 'users.rq');
    });

    it('honours an explicitly selected direct transport', async () => {
        setWasmTransport('direct');
        mockLint.mockReturnValue('{"ok":true,"diagnostics":[]}');

        await wasmCall('lint', ['{}', 'x', 'users.rq']);

        expect(mockLint).toHaveBeenCalled();
    });

    it('surfaces wasm failures as rejections', async () => {
        setWasmTransport('direct');
        mockLint.mockImplementation(() => { throw new Error('wasm exploded'); });

        await expect(wasmCall('lint', ['{}', 'x', 'users.rq'])).rejects.toThrow('wasm exploded');
    });
});
