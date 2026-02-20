import * as cp from 'child_process';
import { EventEmitter } from 'events';
import { Readable } from 'stream';

class MockChildProcess extends EventEmitter {
    stdout: Readable;
    stderr: Readable;
    
    constructor() {
        super();
        this.stdout = new Readable({ read() {} });
        this.stderr = new Readable({ read() {} });
    }
}

export class ShellMock {
    private handlers: Array<{
        pattern: string | RegExp;
        response: { stdout: string; stderr: string; error: Error | null };
    }> = [];

    constructor() {
        this.setupMock();
    }

    private setupMock() {
        // Mock spawn instead of exec
        const spawnMock = cp.spawn as unknown as jest.Mock;
        spawnMock.mockImplementation((command: string, args: string[] = [], options: any) => {
            const fullCmd = `${command} ${args.join(' ')}`;
            
            const handler = this.handlers.find(h => 
                typeof h.pattern === 'string' ? fullCmd.includes(h.pattern) : h.pattern.test(fullCmd)
            );

            const mockProc = new MockChildProcess();

            // Execute logic on next tick to simulate async process
            setTimeout(() => {
                if (handler) {
                    if (handler.response.stdout) {
                        mockProc.stdout.push(handler.response.stdout);
                    }
                    mockProc.stdout.push(null); // End of stream

                    if (handler.response.stderr) {
                        mockProc.stderr.push(handler.response.stderr);
                    }
                    mockProc.stderr.push(null);

                    if (handler.response.error) {
                         // Should emit error? Or just exit with code? 
                         // spawn emits 'error' if process fails to spawn.
                         // But if command runs and fails, it returns exit code != 0.
                         // ShellMock logic for exec typically returns error for exit code.
                         // But for spawn, we usually handle exit code relative to stderr.
                         // Let's assume error means valid execution but failure (exit code 1)
                         // But wait, my spawnAsync wrapper treats exit code != 0 as rejection.
                         
                         // If we want to simulate spawn failure (binary not found):
                         // mockProc.emit('error', handler.response.error);
                         
                         // If we want to simulate non-zero exit:
                         mockProc.emit('close', 1);
                    } else {
                        mockProc.emit('close', 0);
                    }
                } else {
                    const msg = `Unexpected command: ${fullCmd}`;
                    mockProc.stderr.push(msg);
                    mockProc.stderr.push(null);
                    mockProc.emit('close', 1);
                    // Or emit error if we want strict failure
                    // mockProc.emit('error', new Error(msg));
                }
            }, 0);

            return mockProc as unknown as cp.ChildProcess;
        });
    }

    setCommandOutput(pattern: string | RegExp, output: any) {
        this.handlers.push({
            pattern,
            response: {
                stdout: typeof output === 'string' ? output : JSON.stringify(output),
                stderr: '',
                error: null
            }
        });
    }

    setCommandSuccessWithStderr(pattern: string | RegExp, output: any, stderr: string) {
        this.handlers.push({
            pattern,
            response: {
                stdout: typeof output === 'string' ? output : JSON.stringify(output),
                stderr: stderr,
                error: null
            }
        });
    }

    setCommandError(pattern: string | RegExp, errorMessage: string) {
        this.handlers.push({
            pattern,
            response: {
                stdout: '',
                stderr: errorMessage,
                error: new Error(errorMessage)
            }
        });
    }
    
    reset() {
        this.handlers = [];
        (cp.spawn as unknown as jest.Mock).mockClear();
    }
}
