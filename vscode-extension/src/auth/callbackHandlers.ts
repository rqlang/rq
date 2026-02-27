import * as vscode from 'vscode';
import { URL, URLSearchParams } from 'url';
import type { IncomingMessage, ServerResponse } from 'http';

export interface CallbackResult {
    code: string;
    state: string | null;
}

import { waitForUri } from './authUriHandler';

export async function handleVSCodeUriCallback(authUrl: string): Promise<CallbackResult> {
    console.log('Opening authorization URL in browser...');
    vscode.env.openExternal(vscode.Uri.parse(authUrl));

    try {
        // Wait for URI callback using the global handler
        const uri = await waitForUri((uri) => {
            // Determine if this URI is for us (naive check: does it have code or error?)
            const query = new URLSearchParams(uri.query);
            return query.has('code') || query.has('error');
        });

        console.log('Received URI callback:', uri.toString());
        const query = new URLSearchParams(uri.query);
        const code = query.get('code');
        const state = query.get('state');
        const error = query.get('error');
        const errorDescription = query.get('error_description');

        if (error) {
            vscode.window.showErrorMessage(`Authentication error: ${error} - ${errorDescription || ''}`);
            throw new Error(`OAuth error: ${error} - ${errorDescription}`);
        }

        if (code) {
            console.log('Authorization code received via URI handler');
            return { code, state };
        }

        throw new Error('No authorization code in callback URI');
    } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Unknown error';
        throw new Error(`Failed to handle callback URI: ${errorMessage}`);
    }
}

export async function handleLocalServerCallback(authUrl: string, redirectUri: string): Promise<CallbackResult> {
    const http = await import('http');
    const { URL } = await import('url');
    
    return new Promise((resolve, reject) => {
        const server = http.createServer((req: IncomingMessage, res: ServerResponse) => {
            const url = new URL(req.url || '', redirectUri);
            
            if (url.pathname === new URL(redirectUri).pathname) {
                const code = url.searchParams.get('code');
                const state = url.searchParams.get('state');
                const error = url.searchParams.get('error');
                const errorDescription = url.searchParams.get('error_description');

                if (error) {
                    res.writeHead(400, { 'Content-Type': 'text/html' });
                    res.end(`
                        <html>
                        <body style="font-family: Arial, sans-serif; padding: 40px; text-align: center;">
                            <h1 style="color: #c00;">Authentication Error</h1>
                            <p><strong>Error:</strong> ${error}</p>
                            <p>${errorDescription || ''}</p>
                            <p>You can close this window.</p>
                        </body>
                        </html>
                    `);
                    server.close();
                    reject(new Error(`OAuth error: ${error} - ${errorDescription}`));
                    return;
                }

                if (code) {
                    res.writeHead(200, { 'Content-Type': 'text/html' });
                    res.end(`
                        <html>
                        <body style="font-family: Arial, sans-serif; padding: 40px; text-align: center;">
                            <h1 style="color: #0a0;">Authentication Successful!</h1>
                            <p>You have been authenticated successfully.</p>
                            <p>You can close this window and return to VS Code.</p>
                        </body>
                        </html>
                    `);
                    server.close();
                    console.log('Authorization code received via local server');
                    resolve({ code, state });
                    return;
                }

                res.writeHead(400, { 'Content-Type': 'text/html' });
                res.end(`
                    <html>
                    <body style="font-family: Arial, sans-serif; padding: 40px; text-align: center;">
                        <h1 style="color: #c00;">Invalid Response</h1>
                        <p>No authorization code received.</p>
                        <p>You can close this window.</p>
                    </body>
                    </html>
                `);
                server.close();
                reject(new Error('No authorization code in callback'));
            }
        });

        const redirectUrl = new URL(redirectUri);
        const port = parseInt(redirectUrl.port) || 3000;

        server.listen(port, () => {
            console.log(`Local callback server started on ${redirectUri}`);
            vscode.env.openExternal(vscode.Uri.parse(authUrl));
        });

        setTimeout(() => {
            server.close();
            reject(new Error('Authentication timeout - no response received within 5 minutes'));
        }, 5 * 60 * 1000);
    });
}

export async function handleManualPasteCallback(authUrl: string, redirectUri: string): Promise<CallbackResult> {
    console.log('Opening authorization URL in browser...');
    await vscode.env.openExternal(vscode.Uri.parse(authUrl));

    vscode.window.showInformationMessage(
        'Complete the authentication in your browser, then paste the callback URL here.',
        'Open Browser Again'
    ).then(action => {
        if (action === 'Open Browser Again') {
            vscode.env.openExternal(vscode.Uri.parse(authUrl));
        }
    });

    const callbackUrl = await vscode.window.showInputBox({
        prompt: 'Paste the full callback URL from your browser',
        placeHolder: `${redirectUri}?code=...`,
        ignoreFocusOut: true,
        validateInput: (value) => {
            if (!value) {
                return 'Please paste the callback URL';
            }
            if (!value.startsWith(redirectUri)) {
                return `URL must start with ${redirectUri}`;
            }
            return null;
        }
    });

    if (!callbackUrl) {
        throw new Error('Authentication cancelled - no callback URL provided');
    }

    const url = new URL(callbackUrl);
    const code = url.searchParams.get('code');
    const state = url.searchParams.get('state');
    const error = url.searchParams.get('error');
    const errorDescription = url.searchParams.get('error_description');

    if (error) {
        vscode.window.showErrorMessage(`Authentication error: ${error} - ${errorDescription || ''}`);
        throw new Error(`OAuth error: ${error} - ${errorDescription}`);
    }

    if (code) {
        console.log('Authorization code received via manual paste');
        return { code, state };
    }

    throw new Error('No authorization code found in callback URL');
}

export interface ImplicitCallbackResult {
    accessToken: string;
    tokenType?: string;
    expiresIn?: number;
    state: string | null;
}

export async function handleVSCodeUriImplicitCallback(authUrl: string): Promise<ImplicitCallbackResult> {
    console.log('Opening authorization URL in browser...');
    vscode.env.openExternal(vscode.Uri.parse(authUrl));

    try {
        // Wait for URI callback using the global handler
        const uri = await waitForUri((uri) => {
            // Determine if this URI is for implicit flow
            const fragmentParams = new URLSearchParams(uri.fragment);
            const queryParams = new URLSearchParams(uri.query);
            const hasParam = (key: string) => fragmentParams.has(key) || queryParams.has(key);
            
            return hasParam('access_token') || hasParam('error');
        });

        console.log('Received URI callback (Implicit):', uri.toString());
        
        // Parse both fragment and query
        const fragmentParams = new URLSearchParams(uri.fragment);
        const queryParams = new URLSearchParams(uri.query);
        
        // Helper to get value from either source (fragment takes precedence)
        const getParam = (key: string) => fragmentParams.get(key) || queryParams.get(key);

        const accessToken = getParam('access_token');
        const tokenType = getParam('token_type') || 'Bearer';
        const expiresIn = getParam('expires_in');
        const state = getParam('state');
        const error = getParam('error');
        const errorDescription = getParam('error_description');

        if (error) {
            throw new Error(`OAuth error: ${error} - ${errorDescription}`);
        }

        if (accessToken) {
            console.log('Access token received via URI handler');
            return { 
                accessToken, 
                tokenType, 
                expiresIn: expiresIn ? parseInt(expiresIn, 10) : undefined,
                state 
            };
        }

        throw new Error('No access token in callback URI');
    } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Unknown error';
        throw new Error(`Failed to handle callback URI: ${errorMessage}`);
    }
}
