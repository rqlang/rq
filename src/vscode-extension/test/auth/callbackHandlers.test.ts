import * as vscode from 'vscode';
import { handleVSCodeUriImplicitCallback, handleVSCodeUriCallback } from '../../src/auth/callbackHandlers';
import { authUriHandler } from '../../src/auth/authUriHandler';

// Mock VS Code
jest.mock('vscode', () => ({
    Uri: {
        parse: (url: string) => ({
            toString: () => url,
            query: url.split('?')[1] || '',
            fragment: url.split('#')[1] || ''
        })
    },
    env: {
        openExternal: jest.fn().mockResolvedValue(true)
    },
    window: {
        showErrorMessage: jest.fn()
    }
}));

// Mock AuthUriHandler
jest.mock('../../src/auth/authUriHandler', () => ({
    authUriHandler: {
        handleUri: jest.fn()
    },
    waitForUri: jest.fn()
}));

describe('CallbackHandlers', () => {
    beforeEach(() => {
        jest.clearAllMocks();
    });

    describe('handleVSCodeUriImplicitCallback', () => {
        const authUrl = 'https://auth.example.com/authorize';

        it('should parse access_token from URL fragment', async () => {
            // Mock waitForUri to return our test URI immediately
            const mockUri = {
                toString: () => 'vscode://rq-lang.rq-language/callback#access_token=foo&state=bar',
                query: '',
                fragment: 'access_token=foo&state=bar'
            };
            
            const waitForUriMock = require('../../src/auth/authUriHandler').waitForUri;
            waitForUriMock.mockImplementation(async (predicate: Function) => {
                // Verify predicate accepts this URI
                expect(predicate(mockUri)).toBe(true);
                return mockUri;
            });

            const result = await handleVSCodeUriImplicitCallback(authUrl);

            expect(result).toMatchObject({
                accessToken: 'foo',
                state: 'bar'
            });
            expect(vscode.env.openExternal).toHaveBeenCalled();
        });

        it('should parse access_token from URL query (fallback)', async () => {
            const mockUri = {
                toString: () => 'vscode://rq-lang.rq-language/callback?access_token=foo&state=bar',
                query: 'access_token=foo&state=bar',
                fragment: ''
            };
            
            const waitForUriMock = require('../../src/auth/authUriHandler').waitForUri;
            waitForUriMock.mockImplementation(async (predicate: Function) => {
                expect(predicate(mockUri)).toBe(true);
                return mockUri;
            });

            const result = await handleVSCodeUriImplicitCallback(authUrl);

            expect(result).toMatchObject({
                accessToken: 'foo',
                state: 'bar'
            });
        });

        it('should handle errors in fragment', async () => {
            const mockUri = {
                toString: () => 'vscode://rq-lang.rq-language/callback#error=access_denied&error_description=bad',
                query: '',
                fragment: 'error=access_denied&error_description=bad'
            };

            const waitForUriMock = require('../../src/auth/authUriHandler').waitForUri;
            waitForUriMock.mockImplementation(async (predicate: Function) => {
                expect(predicate(mockUri)).toBe(true);
                return mockUri;
            });

            await expect(handleVSCodeUriImplicitCallback(authUrl))
                .rejects.toThrow('OAuth error: access_denied - bad');
        });
    });

    describe('handleVSCodeUriCallback', () => {
        const authUrl = 'https://auth.example.com/authorize';

        it('should parse code from URL query', async () => {
            const mockUri = {
                toString: () => 'vscode://rq-lang.rq-language/callback?code=foo&state=bar',
                query: 'code=foo&state=bar',
                fragment: ''
            };

            const waitForUriMock = require('../../src/auth/authUriHandler').waitForUri;
            waitForUriMock.mockImplementation(async (predicate: Function) => {
                expect(predicate(mockUri)).toBe(true);
                return mockUri;
            });

            const result = await handleVSCodeUriCallback(authUrl);

            expect(result).toEqual({
                code: 'foo',
                state: 'bar'
            });
        });

        it('should handle errors in query', async () => {
            const mockUri = {
                toString: () => 'vscode://rq-lang.rq-language/callback?error=invalid_request',
                query: 'error=invalid_request',
                fragment: ''
            };

            const waitForUriMock = require('../../src/auth/authUriHandler').waitForUri;
            waitForUriMock.mockImplementation(async (predicate: Function) => {
                expect(predicate(mockUri)).toBe(true);
                return mockUri;
            });

            await expect(handleVSCodeUriCallback(authUrl))
                .rejects.toThrow('OAuth error: invalid_request - null');
        });
    });
});