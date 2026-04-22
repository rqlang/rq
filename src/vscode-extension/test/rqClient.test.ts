import * as https from 'https';
import * as fs from 'fs';

const mockGetRequestDetails = jest.fn();
const mockVersion = jest.fn().mockReturnValue('0.0.0-test');

jest.mock('../src/wasm/rq_wasm', () => ({
    get_request_details: mockGetRequestDetails,
    version: mockVersion,
}), { virtual: true });

jest.mock('fs');
jest.mock('https');

import { executeRequest } from '../src/rqClient';

const BASE_DETAILS = {
    Request: 'test-req',
    URL: 'https://example.com/api',
    Method: 'GET',
    Headers: {},
    file: 'test.rq',
    line: 0,
    character: 0,
};

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
    describe('variable passing', () => {
        test('passes variables as key=value JSON array to get_request_details', async () => {
            mockGetRequestDetails.mockReturnValue(JSON.stringify(BASE_DETAILS));
            makeHttpsMock();

            await executeRequest({
                requestName: 'test-req',
                sourceDirectory: '/tmp/project',
                variables: { userId: '123', name: 'Alice' },
            });

            expect(mockGetRequestDetails).toHaveBeenCalledWith(
                expect.any(String),
                expect.any(String),
                expect.any(String),
                'test-req',
                undefined,
                true,
                false,
                JSON.stringify(['userId=123', 'name=Alice']),
            );
        });

        test('passes undefined variables_json when no variables provided', async () => {
            mockGetRequestDetails.mockReturnValue(JSON.stringify(BASE_DETAILS));
            makeHttpsMock();

            await executeRequest({
                requestName: 'test-req',
                sourceDirectory: '/tmp/project',
            });

            expect(mockGetRequestDetails).toHaveBeenCalledWith(
                expect.any(String),
                expect.any(String),
                expect.any(String),
                'test-req',
                undefined,
                true,
                false,
                undefined,
            );
        });

        test('passes environment to get_request_details', async () => {
            mockGetRequestDetails.mockReturnValue(JSON.stringify(BASE_DETAILS));
            makeHttpsMock();

            await executeRequest({
                requestName: 'test-req',
                sourceDirectory: '/tmp/project',
                environment: 'production',
            });

            expect(mockGetRequestDetails).toHaveBeenCalledWith(
                expect.any(String),
                expect.any(String),
                expect.any(String),
                'test-req',
                'production',
                true,
                false,
                undefined,
            );
        });
    });

    describe('default headers', () => {
        test('adds user-agent header when not present in request', async () => {
            mockGetRequestDetails.mockReturnValue(JSON.stringify({ ...BASE_DETAILS, Headers: {} }));
            const { mockRequest } = makeHttpsMock();

            await executeRequest({ requestName: 'test-req', sourceDirectory: '/tmp/project' });

            const callArgs = (https.request as jest.Mock).mock.calls[0][0];
            expect(callArgs.headers).toMatchObject({ 'user-agent': expect.stringMatching(/^rq\//) });
        });

        test('does not override existing user-agent header', async () => {
            mockGetRequestDetails.mockReturnValue(JSON.stringify({
                ...BASE_DETAILS,
                Headers: { 'user-agent': 'custom-agent/1.0' },
            }));
            makeHttpsMock();

            await executeRequest({ requestName: 'test-req', sourceDirectory: '/tmp/project' });

            const callArgs = (https.request as jest.Mock).mock.calls[0][0];
            expect(callArgs.headers['user-agent']).toBe('custom-agent/1.0');
        });

        test('adds content-type application/json for JSON body when not present', async () => {
            mockGetRequestDetails.mockReturnValue(JSON.stringify({
                ...BASE_DETAILS,
                Method: 'POST',
                Body: '{"key":"value"}',
                Headers: {},
            }));
            makeHttpsMock();

            await executeRequest({ requestName: 'test-req', sourceDirectory: '/tmp/project' });

            const callArgs = (https.request as jest.Mock).mock.calls[0][0];
            expect(callArgs.headers['content-type']).toBe('application/json');
        });

        test('does not add content-type for non-JSON body', async () => {
            mockGetRequestDetails.mockReturnValue(JSON.stringify({
                ...BASE_DETAILS,
                Method: 'POST',
                Body: 'plain text body',
                Headers: {},
            }));
            makeHttpsMock();

            await executeRequest({ requestName: 'test-req', sourceDirectory: '/tmp/project' });

            const callArgs = (https.request as jest.Mock).mock.calls[0][0];
            expect(callArgs.headers['content-type']).toBeUndefined();
        });

        test('does not override existing content-type for JSON body', async () => {
            mockGetRequestDetails.mockReturnValue(JSON.stringify({
                ...BASE_DETAILS,
                Method: 'POST',
                Body: '{"key":"value"}',
                Headers: { 'content-type': 'application/vnd.api+json' },
            }));
            makeHttpsMock();

            await executeRequest({ requestName: 'test-req', sourceDirectory: '/tmp/project' });

            const callArgs = (https.request as jest.Mock).mock.calls[0][0];
            expect(callArgs.headers['content-type']).toBe('application/vnd.api+json');
        });
    });

    describe('response mapping', () => {
        test('returns execution result with correct shape', async () => {
            mockGetRequestDetails.mockReturnValue(JSON.stringify({
                ...BASE_DETAILS,
                URL: 'https://example.com/users',
                Method: 'GET',
            }));
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
});
