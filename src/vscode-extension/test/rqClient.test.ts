import * as https from 'https';
import * as fs from 'fs';

const mockPrepareRequest = jest.fn();
const mockSetDebugLogging = jest.fn();
const mockListVariableRefs = jest.fn();
const mockListEnvironments = jest.fn();
const mockListAuth = jest.fn();

jest.mock('../src/wasm/rq_wasm', () => ({
    prepare_request: mockPrepareRequest,
    set_debug_logging: mockSetDebugLogging,
    list_variable_refs: mockListVariableRefs,
    list_environments: mockListEnvironments,
    list_auth: mockListAuth,
}), { virtual: true });

const mockBuildSecretsMap = jest.fn(async () => JSON.stringify({ env_file: null, os_vars: [] }));

jest.mock('../src/utils', () => ({
    ...jest.requireActual('../src/utils'),
    buildSecretsMap: mockBuildSecretsMap,
}));

jest.mock('fs');
jest.mock('https');

import * as vscode from 'vscode';
import { executeRequest, listAuthConfigsWithErrors, listEnvironments, listEnvironmentsWithErrors, setDebugLogging, varRefs } from '../src/rqClient';

const BASE_PREPARED = {
    name: 'test-req',
    url: 'https://example.com/api',
    method: 'GET',
    headers: [] as [string, string][],
    body: null as string | null,
    timeout_ms: null as number | null,
    auth: null as unknown,
};

const PENDING_AUTH = {
    name: 'login',
    auth_type: 'oauth2_authorization_code',
    fields: { client_id: 'app' },
    file: '/tmp/project/auth.rq',
    line: 0,
    character: 0,
    token_required: true,
};

function prepared(overrides: Partial<typeof BASE_PREPARED> = {}): string {
    return JSON.stringify({ ...BASE_PREPARED, ...overrides });
}

function makeHttpsMock(statusCode = 200) {
    const mockResponse = {
        statusCode,
        headers: { 'content-type': 'application/json' },
        on: jest.fn(),
    };
    mockResponse.on.mockImplementation((event: string, cb: (...args: any[]) => void) => {
        if (event === 'data') cb(Buffer.from('{}'));
        if (event === 'end') cb();
        return mockResponse;
    });
    const mockRequest = { on: jest.fn().mockReturnThis(), write: jest.fn().mockReturnThis(), end: jest.fn() };
    (https.request as jest.Mock).mockImplementation((_opts: any, cb: (res: any) => void) => {
        cb(mockResponse);
        return mockRequest;
    });
    return { mockResponse, mockRequest };
}

beforeEach(() => {
    jest.clearAllMocks();
    (fs.statSync as jest.Mock).mockReturnValue({ isDirectory: () => true });
    (fs.readdirSync as jest.Mock).mockReturnValue([]);
    (fs.readFileSync as jest.Mock).mockImplementation((p: string) => {
        if (String(p).endsWith('.env')) throw new Error('no .env');
        return '';
    });
});

describe('executeRequest', () => {
    describe('cancellation', () => {
        test('rejects without sending when cancellation was already requested', async () => {
            mockPrepareRequest.mockReturnValue(prepared());
            makeHttpsMock();

            await expect(executeRequest({
                requestName: 'test-req',
                sourceDirectory: '/tmp/project',
                cancellation: { isCancellationRequested: true, onCancellationRequested: () => ({ dispose: () => {} }) },
            })).rejects.toThrow('Cancelled by user');

            expect(https.request).not.toHaveBeenCalled();
        });

        test('destroys the in-flight request when cancellation fires', async () => {
            mockPrepareRequest.mockReturnValue(prepared());

            const listeners: (() => void)[] = [];
            const cancellation = {
                isCancellationRequested: false,
                onCancellationRequested: (listener: () => void) => {
                    listeners.push(listener);
                    return { dispose: () => {} };
                },
            };

            const handlers: Record<string, (err: Error) => void> = {};
            const mockRequest: any = {
                on: jest.fn().mockImplementation((event: string, cb: (err: Error) => void) => {
                    handlers[event] = cb;
                    return mockRequest;
                }),
                write: jest.fn().mockReturnThis(),
                end: jest.fn(),
                destroy: jest.fn().mockImplementation((err: Error) => handlers['error']?.(err)),
            };
            (https.request as jest.Mock).mockImplementation(() => mockRequest);

            const pending = executeRequest({ requestName: 'test-req', sourceDirectory: '/tmp/project', cancellation });
            await new Promise(resolve => setImmediate(resolve));
            listeners.forEach(listener => listener());

            await expect(pending).rejects.toThrow('Cancelled by user');
            expect(mockRequest.destroy).toHaveBeenCalled();
        });

        test('disposes the cancellation subscription once the response completes', async () => {
            mockPrepareRequest.mockReturnValue(prepared());
            makeHttpsMock();
            const dispose = jest.fn();

            await executeRequest({
                requestName: 'test-req',
                sourceDirectory: '/tmp/project',
                cancellation: { isCancellationRequested: false, onCancellationRequested: () => ({ dispose }) },
            });

            expect(dispose).toHaveBeenCalled();
        });
    });

    describe('variable passing', () => {
        test('passes variables as key=value JSON array to prepare_request', async () => {
            mockPrepareRequest.mockReturnValue(prepared());
            makeHttpsMock();

            await executeRequest({
                requestName: 'test-req',
                sourceDirectory: '/tmp/project',
                variables: { userId: '123', name: 'Alice' },
            });

            expect(mockPrepareRequest).toHaveBeenCalledWith(
                expect.any(String),
                expect.any(String),
                '/tmp/project',
                'test-req',
                undefined,
                JSON.stringify(['userId=123', 'name=Alice']),
                undefined,
            );
        });

        test('passes undefined variables_json when no variables provided', async () => {
            mockPrepareRequest.mockReturnValue(prepared());
            makeHttpsMock();

            await executeRequest({ requestName: 'test-req', sourceDirectory: '/tmp/project' });

            expect(mockPrepareRequest).toHaveBeenCalledWith(
                expect.any(String),
                expect.any(String),
                '/tmp/project',
                'test-req',
                undefined,
                undefined,
                undefined,
            );
        });

        test('passes environment to prepare_request', async () => {
            mockPrepareRequest.mockReturnValue(prepared());
            makeHttpsMock();

            await executeRequest({
                requestName: 'test-req',
                sourceDirectory: '/tmp/project',
                environment: 'production',
            });

            expect(mockPrepareRequest).toHaveBeenCalledWith(
                expect.any(String),
                expect.any(String),
                '/tmp/project',
                'test-req',
                'production',
                undefined,
                undefined,
            );
        });
    });

    describe('source', () => {
        test('prepares the request in the opened folder, scoped to the file that declares it', async () => {
            mockPrepareRequest.mockReturnValue(prepared());
            makeHttpsMock();

            await executeRequest({
                requestName: 'test-req',
                sourceDirectory: '/tmp/project',
                sourceFile: '/tmp/project/sub/main.rq',
            });

            const call = mockPrepareRequest.mock.calls[0];
            expect([call[2], call[6]]).toEqual(['/tmp/project', '/tmp/project/sub/main.rq']);
        });

        test('sends no files of its own so rq reads the opened folder from disk', async () => {
            mockPrepareRequest.mockReturnValue(prepared());
            makeHttpsMock();

            await executeRequest({
                requestName: 'test-req',
                sourceDirectory: '/tmp/project',
                sourceFile: '/tmp/project/sub/main.rq',
            });

            expect(mockPrepareRequest.mock.calls[0][0]).toBe('{}');
        });

        test('reads secrets from the opened folder, not from the request file', async () => {
            mockPrepareRequest.mockReturnValue(prepared());
            makeHttpsMock();

            await executeRequest({
                requestName: 'test-req',
                sourceDirectory: '/tmp/project',
                sourceFile: '/tmp/project/sub/main.rq',
            });

            expect(mockBuildSecretsMap).toHaveBeenCalledWith('/tmp/project');
        });
    });

    describe('prepared request', () => {
        test('sends the headers prepared by rq-lib without adding any', async () => {
            mockPrepareRequest.mockReturnValue(prepared({
                method: 'POST',
                body: '{"key":"value"}',
                headers: [['user-agent', 'rq/1.0.0'], ['x-custom', 'yes']],
            }));
            makeHttpsMock();

            await executeRequest({ requestName: 'test-req', sourceDirectory: '/tmp/project' });

            const callArgs = (https.request as jest.Mock).mock.calls[0][0];
            expect(callArgs.headers).toEqual({
                'user-agent': 'rq/1.0.0',
                'x-custom': 'yes',
                'content-length': '15',
            });
        });

        test('sends every value of a repeated header', async () => {
            mockPrepareRequest.mockReturnValue(prepared({
                headers: [['Accept', 'text/plain'], ['accept', 'application/json']],
            }));
            makeHttpsMock();

            await executeRequest({ requestName: 'test-req', sourceDirectory: '/tmp/project' });

            const callArgs = (https.request as jest.Mock).mock.calls[0][0];
            expect(callArgs.headers.Accept).toEqual(['text/plain', 'application/json']);
        });

        test('reports the values of a repeated header joined', async () => {
            mockPrepareRequest.mockReturnValue(prepared({
                headers: [['Accept', 'text/plain'], ['accept', 'application/json']],
            }));
            makeHttpsMock();

            const target = await executeRequest({ requestName: 'test-req', sourceDirectory: '/tmp/project' });

            expect(target.results[0].request_headers.Accept).toBe('text/plain, application/json');
        });
    });

    describe('auth', () => {
        test('prepares again with the acquired token when rq-lib asks for one', async () => {
            mockPrepareRequest
                .mockReturnValueOnce(prepared({ auth: PENDING_AUTH }))
                .mockReturnValueOnce(prepared({
                    headers: [['authorization', 'Bearer acquired']],
                    auth: { ...PENDING_AUTH, token_required: false },
                }));
            makeHttpsMock();

            await executeRequest({
                requestName: 'test-req',
                sourceDirectory: '/tmp/project',
                acquireToken: async () => 'acquired',
            });

            expect(mockPrepareRequest.mock.calls[1][5]).toBe(JSON.stringify(['auth_token=acquired']));
        });

        test('hands the selected environment to the token source', async () => {
            mockPrepareRequest.mockReturnValue(prepared({ auth: PENDING_AUTH }));
            makeHttpsMock();
            const acquireToken = jest.fn().mockResolvedValue('acquired');

            await executeRequest({
                requestName: 'test-req',
                sourceDirectory: '/tmp/project',
                environment: 'dev',
                acquireToken,
            });

            expect(acquireToken).toHaveBeenCalledWith(expect.objectContaining({ name: 'login', environment: 'dev' }));
        });

        test('does not ask for a token when rq-lib already applied one', async () => {
            mockPrepareRequest.mockReturnValue(prepared({
                headers: [['authorization', 'Bearer known']],
                auth: { ...PENDING_AUTH, token_required: false },
            }));
            makeHttpsMock();
            const acquireToken = jest.fn();

            await executeRequest({ requestName: 'test-req', sourceDirectory: '/tmp/project', acquireToken });

            expect(acquireToken).not.toHaveBeenCalled();
        });

        test('refuses to send a request whose token cannot be obtained', async () => {
            mockPrepareRequest.mockReturnValue(prepared({ auth: PENDING_AUTH }));
            makeHttpsMock();

            await expect(executeRequest({ requestName: 'test-req', sourceDirectory: '/tmp/project' }))
                .rejects.toThrow("Auth configuration 'login' (oauth2_authorization_code) requires a token that could not be obtained");
            expect(https.request).not.toHaveBeenCalled();
        });
    });

    describe('response mapping', () => {
        test('returns execution result with correct shape', async () => {
            mockPrepareRequest.mockReturnValue(prepared({ url: 'https://example.com/users' }));
            makeHttpsMock(201);

            const result = await executeRequest({ requestName: 'test-req', sourceDirectory: '/tmp/project' });

            expect(result.results).toHaveLength(1);
            expect(result.results[0]).toMatchObject({
                request_name: 'test-req',
                method: 'GET',
                url: 'https://example.com/users',
                status: 201,
            });
        });
    });

    describe('timeout', () => {
        test('triggers req.destroy with timeout error when request exceeds configured timeout', async () => {
            mockPrepareRequest.mockReturnValue(prepared({ timeout_ms: 10 }));

            let errorHandler: ((err: Error) => void) | undefined;
            const mockRequest = {
                on: jest.fn().mockImplementation((event: string, cb: (err: Error) => void) => {
                    if (event === 'error') errorHandler = cb;
                    return mockRequest;
                }),
                write: jest.fn(),
                end: jest.fn(),
                destroy: jest.fn().mockImplementation((err: Error) => { errorHandler?.(err); }),
            };
            (https.request as jest.Mock).mockReturnValue(mockRequest);

            await expect(executeRequest({
                requestName: 'test-req',
                sourceDirectory: '/tmp/project',
            })).rejects.toThrow('Request timed out after 10ms');

            expect(mockRequest.destroy).toHaveBeenCalledWith(
                expect.objectContaining({ message: 'Request timed out after 10ms' }),
            );
        });

        test('waits without a timer when the timeout exceeds what a timer can hold', async () => {
            mockPrepareRequest.mockReturnValue(prepared({ timeout_ms: 1e15 }));
            const setTimeoutSpy = jest.spyOn(global, 'setTimeout');
            makeHttpsMock();

            await executeRequest({ requestName: 'test-req', sourceDirectory: '/tmp/project' });

            expect(setTimeoutSpy).not.toHaveBeenCalledWith(expect.any(Function), 1e15);
            setTimeoutSpy.mockRestore();
        });

        test('cancels timeout timer on successful response so destroy is not called', async () => {
            mockPrepareRequest.mockReturnValue(prepared({ timeout_ms: 1000 }));

            const mockResponse = {
                statusCode: 200,
                headers: {},
                on: jest.fn().mockImplementation((event: string, cb: (...args: any[]) => void) => {
                    if (event === 'data') cb(Buffer.from('{}'));
                    if (event === 'end') cb();
                    return mockResponse;
                }),
            };
            const mockRequest = {
                on: jest.fn().mockReturnThis(),
                write: jest.fn().mockReturnThis(),
                end: jest.fn(),
                destroy: jest.fn(),
            };
            (https.request as jest.Mock).mockImplementation((_opts: any, cb: (res: any) => void) => {
                process.nextTick(() => cb(mockResponse));
                return mockRequest;
            });

            await executeRequest({ requestName: 'test-req', sourceDirectory: '/tmp/project' });

            expect(mockRequest.destroy).not.toHaveBeenCalled();
        });

        test('cancels timeout timer on request error so timeout does not fire additionally', async () => {
            mockPrepareRequest.mockReturnValue(prepared({ timeout_ms: 1000 }));

            let reqErrorHandler: ((err: Error) => void) | undefined;
            const mockRequest = {
                on: jest.fn().mockImplementation((event: string, cb: (err: Error) => void) => {
                    if (event === 'error') reqErrorHandler = cb;
                    return mockRequest;
                }),
                write: jest.fn(),
                end: jest.fn(),
                destroy: jest.fn(),
            };
            (https.request as jest.Mock).mockImplementation(() => {
                process.nextTick(() => reqErrorHandler?.(new Error('connection refused')));
                return mockRequest;
            });

            await expect(executeRequest({
                requestName: 'test-req',
                sourceDirectory: '/tmp/project',
            })).rejects.toThrow('connection refused');

            expect(mockRequest.destroy).not.toHaveBeenCalled();
        });
    });
});

describe('setDebugLogging', () => {
    it('toggles the library debug trace', async () => {
        await setDebugLogging(true);

        expect(mockSetDebugLogging).toHaveBeenCalledWith(true);
    });
});

describe('varRefs', () => {
    function makeDocument(uri: any, text: string, isUntitled = false) {
        return { uri, isUntitled, languageId: 'rq', isDirty: true, getText: () => text } as any;
    }

    afterEach(() => {
        (vscode.workspace as any).textDocuments = [];
    });

    test('resolves references against the unsaved text of the document', async () => {
        const doc = makeDocument({ fsPath: '/tmp/project/foo.rq' }, 'rq get(resource_id);');
        mockListVariableRefs.mockReturnValue('[]');

        await varRefs('resource_id', doc, new vscode.Position(0, 8), '/tmp/project');

        expect(JSON.parse(mockListVariableRefs.mock.calls[0][0])).toEqual({
            '/tmp/project/foo.rq': 'rq get(resource_id);',
        });
    });

    test('includes other unsaved documents of the folder', async () => {
        const doc = makeDocument({ fsPath: '/tmp/project/foo.rq' }, 'rq get(base);');
        const other = makeDocument({ fsPath: '/tmp/project/shared.rq' }, 'let base = "x";');
        (vscode.workspace as any).textDocuments = [doc, other];
        mockListVariableRefs.mockReturnValue('[]');

        await varRefs('base', doc, new vscode.Position(0, 8), '/tmp/project');

        expect(JSON.parse(mockListVariableRefs.mock.calls[0][0])).toEqual({
            '/tmp/project/foo.rq': 'rq get(base);',
            '/tmp/project/shared.rq': 'let base = "x";',
        });
    });

    test('passes the cursor position so a required variable stays in its request', async () => {
        const doc = makeDocument({ fsPath: '/tmp/project/foo.rq' }, 'rq get(resource_id);');
        mockListVariableRefs.mockReturnValue('[]');

        await varRefs('resource_id', doc, new vscode.Position(4, 12), '/tmp/project');

        const call = mockListVariableRefs.mock.calls[0];
        expect(call.slice(4)).toEqual(['/tmp/project/foo.rq', 4, 12]);
    });

    test('keeps the Windows drive letter as VS Code reports it', async () => {
        const doc = makeDocument({ fsPath: 'c:\\ws\\foo.rq' }, 'rq get(id);');
        mockListVariableRefs.mockReturnValue('[]');

        await varRefs('id', doc, new vscode.Position(0, 8), 'c:\\ws');

        expect(mockListVariableRefs.mock.calls[0][4]).toBe('c:/ws/foo.rq');
    });

    test('flags references that the endpoint shares with other requests', async () => {
        const uri = { fsPath: '/tmp/project/foo.rq' };
        const doc = makeDocument(uri, 'rq get(id);');
        mockListVariableRefs.mockReturnValue(JSON.stringify([
            { file: '/tmp/project/foo.rq', line: 0, character: 20, shared: true },
        ]));

        const target = await varRefs('id', doc, new vscode.Position(0, 8), '/tmp/project');

        expect(target).toEqual([{ uri, line: 0, character: 20, shared: true }]);
    });

    test('maps references in an untitled document back to that document', async () => {
        const uri = { fsPath: 'Untitled-1', path: 'Untitled-1', scheme: 'untitled' };
        const doc = makeDocument(uri, 'rq get(resource_id);', true);
        mockListVariableRefs.mockReturnValue(JSON.stringify([
            { file: '/tmp/project/Untitled-1', line: 0, character: 7 },
        ]));

        const target = await varRefs('resource_id', doc, new vscode.Position(0, 8), '/tmp/project');

        expect(target).toEqual([{ uri, line: 0, character: 7, shared: false }]);
    });
});

describe('listEnvironmentsWithErrors', () => {
    test('returns the environment names and the files that failed to parse', async () => {
        mockListEnvironments.mockReturnValue(JSON.stringify({
            environments: [{ name: 'local', file: '/tmp/project/env.rq', line: 0, character: 0, color: null }],
            parse_errors: [{ severity: 'error', message: 'Unexpected end of file', line: 2, column: 0, file: '/tmp/project/broken.rq' }],
        }));

        const target = await listEnvironmentsWithErrors('/tmp/project');

        expect(target).toEqual({ environments: ['local'], colors: new Map(), errors: ['Unexpected end of file'] });
    });

    test('returns the ui color of the environments that declare one', async () => {
        mockListEnvironments.mockReturnValue(JSON.stringify({
            environments: [
                { name: 'local', file: '/tmp/project/env.rq', line: 0, character: 0, color: null },
                { name: 'production', file: '/tmp/project/env.rq', line: 5, character: 4, color: 'red' },
            ],
        }));

        const target = await listEnvironmentsWithErrors('/tmp/project');

        expect(target.colors).toEqual(new Map([['production', 'red']]));
    });
});

describe('listEnvironments', () => {
    test('returns only the environment names', async () => {
        mockListEnvironments.mockReturnValue(JSON.stringify({
            environments: [{ name: 'local', file: '/tmp/project/env.rq', line: 0, character: 0 }],
            parse_errors: [{ severity: 'error', message: 'Unexpected end of file', line: 2, column: 0, file: '/tmp/project/broken.rq' }],
        }));

        const target = await listEnvironments('/tmp/project');

        expect(target).toEqual(['local']);
    });
});

describe('listAuthConfigsWithErrors', () => {
    test('returns the auth configs and the files that failed to parse', async () => {
        mockListAuth.mockReturnValue(JSON.stringify({
            auth: [{ name: 'token', auth_type: 'bearer' }],
            parse_errors: [{ severity: 'error', message: 'Unexpected end of file', line: 2, column: 0, file: '/tmp/project/broken.rq' }],
        }));

        const target = await listAuthConfigsWithErrors('/tmp/project');

        expect(target).toEqual({ authConfigs: [{ name: 'token', auth_type: 'bearer' }], errors: ['Unexpected end of file'] });
    });
});
