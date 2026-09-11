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
            token: vscode.CancellationToken,
            vsContext: vscode.CompletionContext
        ) {
            const { ctx, cleanup } = buildContext(document, position, vsContext, environmentProvider);
            try {
                for (const handler of ALL_HANDLERS) {
                    if (handler.canHandle(ctx)) {
                        const items = await handler.provide(ctx);
                        if (!items || token?.isCancellationRequested) { return undefined; }
                        return handler.incomplete ? new vscode.CompletionList(items, true) : items;
                    }
                }
                return undefined;
            } finally {
                cleanup();
            }
        },
    },
    '.', '{', '[', ',', ' ', 'v', 'e', 'p', 'q', ')', '<', '=', '"', ':', '(', '\n', '$'
);
