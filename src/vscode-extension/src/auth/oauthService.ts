import * as vscode from 'vscode';
import * as cliService from '../cliService';
import { OAuthProvider } from './oauthProvider';
import { OAuth2Config } from './types';

/**
 * Determine the OAuth2 flow type from the auth type string
 */
export function getFlowTypeFromAuthType(authType: string): 'authorization_code_pkce' | 'implicit' {
    switch (authType) {
        case 'oauth2_authorization_code':
            return 'authorization_code_pkce';
        case 'oauth2_implicit':
            return 'implicit';
        default:
            throw new Error(
                `Auth type '${authType}' is not supported. ` +
                `Supported types: oauth2_authorization_code (OAuth2 with PKCE), oauth2_implicit`
            );
    }
}

/**
 * Perform OAuth2 flow based on CLI auth configuration
 * 
 * @param authConfig The auth configuration from the CLI
 * @param context The extension context
 * @param outputChannel Optional output channel for logging
 * @returns The access token
 * @raises Error: If OAuth2 flow fails or auth type is unsupported
 */
export async function performOAuth2Flow(
    authConfig: cliService.AuthShowOutput,
    context: vscode.ExtensionContext,
    outputChannel?: vscode.OutputChannel
): Promise<string> {
    try {
        // Determine flow type based on auth configuration
        const flowType = getFlowTypeFromAuthType(authConfig.auth_type);
        
        // Create OAuth provider
        const oauthProvider = new OAuthProvider(context);
        if (outputChannel) {
            oauthProvider.setOutputChannel(outputChannel);
        }
        
        // Log the configuration for debugging
        console.log('OAuth2 Configuration:');
        console.log('  Auth Type:', authConfig.auth_type);
        console.log('  Flow Type:', flowType);
        console.log('  Client ID:', authConfig.fields.client_id);
        console.log('  Authorization URL:', authConfig.fields.authorization_url);
        console.log('  Token URL:', authConfig.fields.token_url);
        console.log('  Redirect URI:', authConfig.fields.redirect_uri);
        console.log('  Scope:', authConfig.fields.scope);
        console.log('  Code Challenge Method:', authConfig.fields.code_challenge_method);
        
        // Validate URLs before creating config
        if (!authConfig.fields.authorization_url) {
            throw new Error('Missing authorization_url in auth configuration');
        }
        
        // Token URL is required for auth code flow but not implicit
        if (flowType === 'authorization_code_pkce' && !authConfig.fields.token_url) {
            throw new Error('Missing token_url in auth configuration');
        }
        
        // Validate URL format
        try {
            new URL(authConfig.fields.authorization_url);
        } catch (error) {
            console.error('Invalid authorization_url:', authConfig.fields.authorization_url);
            throw new Error(`Invalid authorization_url: "${authConfig.fields.authorization_url}". Make sure it's a valid URL with proper quotes in your .rq file.`);
        }
        
        if (authConfig.fields.token_url) {
            try {
                new URL(authConfig.fields.token_url);
            } catch (error) {
                console.error('Invalid token_url:', authConfig.fields.token_url);
                throw new Error(`Invalid token_url: "${authConfig.fields.token_url}". Make sure it's a valid URL with proper quotes in your .rq file.`);
            }
        }
        
        // Extract OAuth2 configuration from CLI auth config
        const config: OAuth2Config = {
            clientId: authConfig.fields.client_id,
            clientSecret: authConfig.fields.client_secret || undefined,
            authorizationUrl: authConfig.fields.authorization_url,
            tokenUrl: authConfig.fields.token_url,
            // Default to VS Code URI handler if not specified
            // This provides the best user experience without requiring a local server
            redirectUri: authConfig.fields.redirect_uri || 'vscode://rq-lang.rq-language/oauth-callback',
            scope: authConfig.fields.scope || undefined,
            codeChallengeMethod: (authConfig.fields.code_challenge_method as 'S256' | 'plain') || 'S256',
            useState: authConfig.fields.use_state !== 'false',
            usePkce: authConfig.fields.use_pkce !== 'false',
        };

        // Execute OAuth2 flow using the dynamically selected flow type
        // Pass auth name and environment for proper token caching per environment
        const result = await oauthProvider.executeOAuth2Flow(
            config, 
            flowType, 
            authConfig.name, 
            authConfig.environment
        );
        
        return result.accessToken;
    } catch (error) {
        const errorMessage = error instanceof Error ? error.message : String(error);
        console.error('OAuth2 flow failed:', errorMessage);
        console.error('Full error:', error);
        
        // NOTE: We don't show window.showErrorMessage here to avoid duplicate notifications
        // The caller (e.g. runRequest) should handle the UI feedback
        
        throw error;
    }
}
