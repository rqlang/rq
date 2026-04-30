import * as vscode from 'vscode';
import { buildContext, EnvironmentProvider } from './completionContext';
import { ALL_HANDLERS } from './completionHandlers';

let environmentProvider: EnvironmentProvider | undefined;

export function setEnvironmentProvider(provider: EnvironmentProvider) {
    environmentProvider = provider;
}

export const completionProvider = vscode.languages.registerCompletionItemProvider(
    'rq',
    {
        async provideCompletionItems(
            document: vscode.TextDocument,
            position: vscode.Position,
            _token: vscode.CancellationToken,
            vsContext: vscode.CompletionContext
        ) {
            const { ctx, cleanup } = buildContext(document, position, vsContext, environmentProvider);
            try {
                for (const handler of ALL_HANDLERS) {
                    if (handler.canHandle(ctx)) {
                        return await handler.provide(ctx);
                    }
                }
                return undefined;
            } finally {
                cleanup();
            }
        },
    },
    '.', '{', '[', ' ', 'v', 'e', 'p', 'q', ')', '<', '=', '"', ':', '(', '\n', '$'
);
