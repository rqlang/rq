import * as vscode from 'vscode';
import * as path from 'path';
import * as rqClient from '../rqClient';
import { CompletionContext, listVariablesWithFallback } from './completionContext';
import {
    builtinFunctionItems,
    dollarPrefixItems,
    propertyItems,
    insideEnvOrAuthBlock,
    insideEpBody,
    insideArrayLiteral,
    insideHeadersLiteral,
    getActiveAuthBlock,
    collectNamedProps,
    countPositionalArgs,
    COMMON_HEADERS,
    AUTH_PROPERTIES,
} from './completionHelpers';
import {
    REQUEST_PROPERTIES,
    ENDPOINT_PROPERTIES,
    parseVariables,
} from './definitions';

export interface CompletionHandler {
    canHandle(ctx: CompletionContext): boolean;
    provide(ctx: CompletionContext): Promise<vscode.CompletionItem[] | undefined>;
}

export const epTemplateHandler: CompletionHandler = {
    canHandle: ({ linePrefix }) => /^\s*ep\s+[a-zA-Z_][a-zA-Z0-9_-]*\s*<$/.test(linePrefix),
    async provide(ctx) {
        try {
            const endpoints = await rqClient.listEndpoints(await ctx.getCliFilePath());
            return endpoints.filter(ep => ep.is_template).map(ep => {
                const item = new vscode.CompletionItem(ep.name, vscode.CompletionItemKind.Reference);
                item.detail = 'Endpoint template';
                item.insertText = ep.name;
                return item;
            });
        } catch {
            return undefined;
        }
    },
};

export const importHandler: CompletionHandler = {
    canHandle: ({ linePrefix }) =>
        /^\s*import$/.test(linePrefix) || /^\s*import\s+$/.test(linePrefix),
    async provide(ctx) {
        const { linePrefix, documentPrefix, document, position } = ctx;
        const hasNonImportContentBefore = /^\s*(let|rq|ep|env|auth)\b/m.test(documentPrefix);
        const inRqOrEp = /\b(rq|ep)\s+\w+\s*\(/.test(documentPrefix);
        if (inRqOrEp || hasNonImportContentBefore) { return undefined; }

        const hasTrailingSpace = /^\s*import\s+$/.test(linePrefix);
        const currentDir = path.dirname(document.uri.fsPath);
        const allFiles = await vscode.workspace.findFiles('**/*.rq');
        const otherFiles = allFiles.filter(u => u.fsPath !== document.uri.fsPath);

        return otherFiles.map(fileUri => {
            const relativePath = path.relative(currentDir, fileUri.fsPath).replace(/\.rq$/, '').replace(/\\/g, '/');
            const item = new vscode.CompletionItem(relativePath, vscode.CompletionItemKind.File);
            item.detail = 'Import .rq file';
            item.insertText = hasTrailingSpace ? `"${relativePath}";` : ` "${relativePath}";`;
            item.commitCharacters = [';'];
            return item;
        });
    },
};

export const dollarPrefixHandler: CompletionHandler = {
    canHandle: ({ linePrefix }) => linePrefix.endsWith('$'),
    async provide(_ctx) {
        return dollarPrefixItems();
    },
};

export const letAssignmentHandler: CompletionHandler = {
    canHandle: ({ linePrefix }) =>
        /^\s*let\s+[a-zA-Z_][a-zA-Z0-9_-]*\s*=\s*$/.test(linePrefix),
    async provide(ctx) {
        const jsonItem = new vscode.CompletionItem('${', vscode.CompletionItemKind.Snippet);
        jsonItem.detail = 'JSON object literal';
        jsonItem.sortText = '0${';
        jsonItem.insertText = new vscode.SnippetString('\\${\n\t${1:}\n}');

        const headersItem = new vscode.CompletionItem('$[', vscode.CompletionItemKind.Snippet);
        headersItem.detail = 'Headers dictionary';
        headersItem.sortText = '0$[';
        headersItem.insertText = new vscode.SnippetString('\\$["${1:Header-Name}": "${2:value}"]');

        const suggestions: vscode.CompletionItem[] = [jsonItem, headersItem, ...builtinFunctionItems()];
        const varItems = await listVariablesWithFallback(ctx);
        varItems.forEach(v => { v.insertText = `${v.label};`; suggestions.push(v); });
        return suggestions;
    },
};

export const interpolationHandler: CompletionHandler = {
    canHandle: ({ linePrefix }) => /\{\{([a-zA-Z0-9_-]*)$/.test(linePrefix),
    async provide(ctx) {
        const { linePrefix, document, position } = ctx;
        const interpolationMatch = linePrefix.match(/\{\{([a-zA-Z0-9_-]*)$/);
        if (!interpolationMatch) { return undefined; }
        const partial = interpolationMatch[1];
        let replaceRange: vscode.Range | undefined;
        if (partial.length > 0) {
            const afterCursor = document.lineAt(position.line).text.substring(position.character);
            const trailingName = afterCursor.match(/^([a-zA-Z0-9_-]*)/)?.[1] ?? '';
            replaceRange = new vscode.Range(
                position.line, position.character - partial.length,
                position.line, position.character + trailingName.length
            );
        }
        try {
            const variables = await rqClient.listVariables(await ctx.getCliFilePath(), ctx.getEnvironment());
            if (variables.length > 0) {
                return variables.map(v => {
                    const item = new vscode.CompletionItem(v.name, vscode.CompletionItemKind.Variable);
                    item.detail = v.value ? `= ${v.value}` : v.source;
                    item.insertText = v.name;
                    if (replaceRange) { item.range = replaceRange; }
                    return item;
                });
            }
        } catch { /* fall through */ }
        const localVars = parseVariables(document);
        if (localVars.length === 0) { return undefined; }
        return localVars.map(v => {
            const item = new vscode.CompletionItem(v.name, vscode.CompletionItemKind.Variable);
            item.detail = `Variable (line ${v.line + 1})`;
            item.documentation = new vscode.MarkdownString(`Value: \`${v.value}\``);
            item.insertText = v.name;
            if (replaceRange) { item.range = replaceRange; }
            return item;
        });
    },
};

export const codeChallengeMethodHandler: CompletionHandler = {
    canHandle: ({ linePrefix }) => /code_challenge_method\s*:\s*"?[^"]*$/.test(linePrefix),
    async provide({ linePrefix }) {
        const hasOpenQuote = /code_challenge_method\s*:\s*"/.test(linePrefix);
        return ['S256', 'plain'].map(val => {
            const item = new vscode.CompletionItem(val, vscode.CompletionItemKind.EnumMember);
            item.insertText = hasOpenQuote ? `${val}"` : `"${val}"`;
            return item;
        });
    },
};

export const propertyValueHandler: CompletionHandler = {
    canHandle: ({ linePrefix }) => /^\s*"?[a-zA-Z_][a-zA-Z0-9_-]*"?\s*:\s*"?$/.test(linePrefix),
    async provide(ctx) {
        const { documentPrefix } = ctx;
        if (!insideEnvOrAuthBlock(documentPrefix) && !insideHeadersLiteral(documentPrefix)) {
            return undefined;
        }
        const suggestions: vscode.CompletionItem[] = [...builtinFunctionItems()];
        const varItems = await listVariablesWithFallback(ctx);
        suggestions.push(...varItems);
        return suggestions;
    },
};

export const authPropertyNameHandler: CompletionHandler = {
    canHandle: ({ linePrefix, documentPrefix }) => {
        if (!/^\s*\w*$/.test(linePrefix)) { return false; }
        return getActiveAuthBlock(documentPrefix) !== null;
    },
    async provide({ documentPrefix }) {
        const authBlock = getActiveAuthBlock(documentPrefix);
        if (!authBlock) { return undefined; }
        const props = AUTH_PROPERTIES[authBlock.authType];
        if (!props) { return undefined; }
        return props
            .filter(p => !authBlock.definedProps.has(p.name))
            .map(p => {
                const item = new vscode.CompletionItem(p.name, vscode.CompletionItemKind.Property);
                item.detail = p.required ? 'required' : 'optional';
                item.insertText = p.name === 'code_challenge_method'
                    ? new vscode.SnippetString('code_challenge_method: "${1|S256,plain|}"')
                    : new vscode.SnippetString(`${p.name}: "\${1:}"`);
                item.sortText = p.required ? `0${p.name}` : `1${p.name}`;
                return item;
            });
    },
};

export const authTypeHandler: CompletionHandler = {
    canHandle: ({ linePrefix }) =>
        linePrefix.endsWith('auth_type.') || /auth_type\.\w*$/.test(linePrefix),
    async provide(_ctx) {
        return [
            { name: 'bearer', detail: 'Bearer Token Authentication', description: 'Simple bearer token authentication. Requires: token' },
            { name: 'oauth2_authorization_code', detail: 'OAuth2 Authorization Code with PKCE', description: 'OAuth2 authorization code flow with PKCE' },
            { name: 'oauth2_client_credentials', detail: 'OAuth2 Client Credentials', description: 'OAuth2 client credentials flow' },
            { name: 'oauth2_implicit', detail: 'OAuth2 Implicit Flow', description: 'OAuth2 implicit flow' },
        ].map(t => {
            const item = new vscode.CompletionItem(t.name, vscode.CompletionItemKind.EnumMember);
            item.detail = t.detail;
            item.documentation = new vscode.MarkdownString(t.description);
            item.insertText = t.name;
            return item;
        });
    },
};

export const authDeclarationHandler: CompletionHandler = {
    canHandle: ({ documentPrefix }) => {
        const m = documentPrefix.match(/\bauth\s+\w+\s*\(([^{;]*)$/s);
        return !!m && !/auth_type\./.test(m[1]);
    },
    async provide(_ctx) {
        const item = new vscode.CompletionItem('auth_type', vscode.CompletionItemKind.EnumMember);
        item.detail = 'Auth type parameter';
        item.insertText = new vscode.SnippetString('auth_type.');
        item.command = { command: 'editor.action.triggerSuggest', title: 'Re-trigger completions' };
        return [item];
    },
};

function buildRqEpHandler(
    blockPattern: RegExp,
    props: typeof REQUEST_PROPERTIES,
    propNames: string[],
    isRq: boolean
): CompletionHandler {
    return {
        canHandle({ documentPrefix, linePrefix }) {
            if (!blockPattern.test(documentPrefix)) { return false; }
            const atStartOfParams = isRq
                ? /\brq\s+\w+\s*\(\s*$/.test(linePrefix)
                : /\bep\s+\w+\s*\(\s*$/.test(linePrefix);
            const afterComma = /,\s*$/.test(linePrefix);
            const onNewLine = /^\s*$/.test(linePrefix);
            const atNamedValue = new RegExp(`\\b(${propNames.join('|')})\\s*:\\s*$`).test(linePrefix);
            return atStartOfParams || afterComma || onNewLine || atNamedValue;
        },
        async provide(ctx) {
            const { documentPrefix, linePrefix } = ctx;
            const matchResult = documentPrefix.match(blockPattern);
            if (!matchResult) { return undefined; }
            const matchedText = matchResult[0];

            const atNamedValue = new RegExp(`\\b(${propNames.join('|')})\\s*:\\s*$`).test(linePrefix);
            if (atNamedValue) {
                const suggestions: vscode.CompletionItem[] = [...builtinFunctionItems()];
                const varItems = await listVariablesWithFallback(ctx);
                suggestions.push(...varItems);
                return suggestions;
            }

            const existingNamed = collectNamedProps(matchedText, propNames);
            const positionalCount = countPositionalArgs(matchedText);
            propNames.slice(0, positionalCount).forEach(p => existingNamed.add(p));

            const hasNamedParams = props.some(p => existingNamed.has(p.name));
            if (hasNamedParams) {
                return propertyItems(props, existingNamed, false);
            }

            const suggestions: vscode.CompletionItem[] = [
                ...builtinFunctionItems(),
                ...propertyItems(props, existingNamed, true),
            ];
            try {
                const variables = await rqClient.listVariables(await ctx.getCliFilePath(), ctx.getEnvironment());
                variables.forEach(v => {
                    const item = new vscode.CompletionItem(v.name, vscode.CompletionItemKind.Variable);
                    item.detail = v.value ? `= ${v.value}` : v.source;
                    item.insertText = v.name;
                    suggestions.push(item);
                });
            } catch { /* ignore */ }
            return suggestions;
        },
    };
}

export const rqBlockHandler = buildRqEpHandler(
    /\brq\s+\w+\s*\([^;]*$/s,
    REQUEST_PROPERTIES,
    ['url', 'headers', 'body', 'method'],
    true
);

export const epBlockHandler = buildRqEpHandler(
    /\bep\s+\w+\s*\([^{;]*$/s,
    ENDPOINT_PROPERTIES,
    ['url', 'headers', 'qs'],
    false
);

export const authNameValueHandler: CompletionHandler = {
    canHandle: ({ linePrefix }) => /^\s*\[auth\("([^"]*)$/.test(linePrefix),
    async provide(ctx) {
        const { linePrefix, document, position } = ctx;
        const m = linePrefix.match(/^\s*\[auth\("([^"]*)$/);
        if (!m) { return undefined; }
        const partial = m[1];
        try {
            const authConfigs = await rqClient.listAuthConfigs(await ctx.getCliFilePath());
            const afterCursor = document.lineAt(position.line).text.substring(position.character);
            const trailingName = afterCursor.match(/^([^"]*)/)?.[1] ?? '';
            return authConfigs.map(a => {
                const item = new vscode.CompletionItem(a.name, vscode.CompletionItemKind.Reference);
                item.detail = a.auth_type;
                item.insertText = a.name;
                item.range = new vscode.Range(
                    position.line, position.character - partial.length,
                    position.line, position.character + trailingName.length
                );
                return item;
            });
        } catch { return undefined; }
    },
};

export const attributeHandler: CompletionHandler = {
    canHandle: ({ linePrefix }) => /^\s*\[(\w*)$/.test(linePrefix),
    async provide(ctx) {
        const { linePrefix, documentPrefix, position } = ctx;
        const bracketOffset = linePrefix.lastIndexOf('[');
        const blockTextBeforeBracket = ctx.document.getText(new vscode.Range(
            new vscode.Position(Math.max(0, position.line - 50), 0),
            new vscode.Position(position.line, bracketOffset)
        ));
        if (insideArrayLiteral(blockTextBeforeBracket) || insideEnvOrAuthBlock(documentPrefix)) {
            return undefined;
        }

        const methodItem = new vscode.CompletionItem('method', vscode.CompletionItemKind.Keyword);
        methodItem.detail = 'Override request method';
        methodItem.documentation = new vscode.MarkdownString('Sets the HTTP method for the next `rq` statement.\n\n**Example:** `[method(POST)]`');
        methodItem.insertText = new vscode.SnippetString('method(${1|GET,POST,PUT,DELETE,PATCH,HEAD,OPTIONS|})');

        const timeoutItem = new vscode.CompletionItem('timeout', vscode.CompletionItemKind.Keyword);
        timeoutItem.detail = 'Request timeout in seconds';
        timeoutItem.documentation = new vscode.MarkdownString('Sets the timeout (in seconds) for the next `rq` statement.\n\n**Example:** `[timeout(30)]`');
        timeoutItem.insertText = new vscode.SnippetString('timeout(${1:30})');

        const authItem = new vscode.CompletionItem('auth', vscode.CompletionItemKind.Keyword);
        authItem.detail = 'Auth provider name';
        authItem.documentation = new vscode.MarkdownString('Attaches an auth provider to the next `rq` statement.\n\n**Example:** `[auth("my_bearer")]`');
        authItem.insertText = new vscode.SnippetString('auth("$1")');
        authItem.command = { command: 'editor.action.triggerSuggest', title: 'Re-trigger completions' };

        const requiredItem = new vscode.CompletionItem('required', vscode.CompletionItemKind.Keyword);
        requiredItem.detail = 'Require variable at runtime';
        requiredItem.documentation = new vscode.MarkdownString('Marks a variable as required.\n\n**Example:** `[required(user_id)]`');
        requiredItem.insertText = new vscode.SnippetString('required(${1:var_name})');

        return [methodItem, timeoutItem, authItem, requiredItem];
    },
};

export const headerKeyHandler: CompletionHandler = {
    canHandle: ({ linePrefix, documentPrefix }) => {
        if (!/^\s*"?([a-zA-Z0-9_-]*)$/.test(linePrefix)) { return false; }
        return insideHeadersLiteral(documentPrefix);
    },
    async provide(ctx) {
        const { linePrefix, document, position } = ctx;
        const headerKeyMatch = linePrefix.match(/^\s*"?([a-zA-Z0-9_-]*)$/);
        if (!headerKeyMatch) { return undefined; }
        const partial = headerKeyMatch[1];
        const hasOpenQuote = /["'][a-zA-Z0-9_-]*$/.test(linePrefix);
        const afterLine = document.lineAt(position.line).text.substring(position.character);
        const trailingKey = afterLine.match(/^([a-zA-Z0-9_-]*)/)?.[1] ?? '';
        const trailingQuote = hasOpenQuote && afterLine.charAt(trailingKey.length) === '"' ? 1 : 0;
        const replaceRange = new vscode.Range(
            position.line, position.character - partial.length - (hasOpenQuote ? 1 : 0),
            position.line, position.character + trailingKey.length + trailingQuote
        );
        return COMMON_HEADERS.map(header => {
            const item = new vscode.CompletionItem(header, vscode.CompletionItemKind.Value);
            item.detail = 'HTTP header';
            item.insertText = new vscode.SnippetString(`"${header}": "\${1:}"`);
            item.range = replaceRange;
            return item;
        });
    },
};

export const namespaceHandler: CompletionHandler = {
    canHandle: ({ linePrefix }) =>
        linePrefix.endsWith('io.') ||
        linePrefix.endsWith('random.') ||
        linePrefix.endsWith('datetime.') ||
        linePrefix.endsWith('sys.'),
    async provide({ linePrefix }) {
        if (linePrefix.endsWith('io.')) {
            return [
                (() => {
                    const i = new vscode.CompletionItem('read_file', vscode.CompletionItemKind.Function);
                    i.detail = 'io.read_file(path: string)';
                    i.documentation = new vscode.MarkdownString('Imports the contents of a file relative to the current .rq file\n\n**Parameters:**\n- path: string - Relative or absolute path to the file to import');
                    i.insertText = new vscode.SnippetString('read_file("${1:path}")');
                    return i;
                })(),
            ];
        }
        if (linePrefix.endsWith('random.')) {
            const i = new vscode.CompletionItem('guid', vscode.CompletionItemKind.Function);
            i.detail = 'random.guid() → string';
            i.documentation = new vscode.MarkdownString('Generates a random GUID (UUID v4)');
            i.insertText = new vscode.SnippetString('guid();');
            return [i];
        }
        if (linePrefix.endsWith('datetime.')) {
            const i = new vscode.CompletionItem('now', vscode.CompletionItemKind.Function);
            i.detail = 'datetime.now(format?: string) → string';
            i.documentation = new vscode.MarkdownString('Returns the current date and time.\n\n**Parameters:**\n- format: string (optional) - The format string (e.g. "%Y-%m-%d")');
            i.insertText = new vscode.SnippetString('now(${1:});');
            return [i];
        }
        return [];
    },
};

export const topLevelKeywordHandler: CompletionHandler = {
    canHandle: ({ linePrefix, triggerKind }) => {
        if (!/^\s*\w*$/.test(linePrefix)) { return false; }
        if (/^\s*$/.test(linePrefix) && triggerKind === vscode.CompletionTriggerKind.TriggerCharacter) { return false; }
        return true;
    },
    async provide(ctx) {
        const { documentPrefix } = ctx;
        const rqMatch = documentPrefix.match(/\brq\s+\w+\s*\([^;]*$/s);
        const epMatch = documentPrefix.match(/\bep\s+\w+\s*\([^{;]*$/s);
        if (rqMatch || epMatch || insideEnvOrAuthBlock(documentPrefix)) { return undefined; }

        const kw = (label: string, detail: string, sort: string) => {
            const i = new vscode.CompletionItem(label, vscode.CompletionItemKind.Keyword);
            i.detail = detail;
            i.insertText = label + ' ';
            i.sortText = sort;
            return i;
        };

        if (insideEpBody(documentPrefix)) {
            return [
                kw('rq', 'HTTP request keyword', 'rq_0kw'),
                (() => {
                    const i = new vscode.CompletionItem('rq …', vscode.CompletionItemKind.Module);
                    i.detail = 'HTTP request snippet';
                    i.insertText = new vscode.SnippetString('rq ${1:rq_name}($0);');
                    i.sortText = 'rq_1sn';
                    return i;
                })(),
            ];
        }

        const hasNonImportContent = /^\s*(let|rq|ep|env|auth)\b/m.test(documentPrefix);
        return [
            kw('auth', 'Auth block keyword', 'auth_0kw'),
            (() => {
                const i = new vscode.CompletionItem('auth bearer', vscode.CompletionItemKind.Module);
                i.detail = 'Auth block — Bearer Token';
                i.insertText = new vscode.SnippetString('auth ${1:my_auth}(auth_type.bearer) {\n\ttoken: "${2:}"\n}');
                i.sortText = 'auth_2bearer';
                return i;
            })(),
            (() => {
                const i = new vscode.CompletionItem('auth oauth2_authorization_code', vscode.CompletionItemKind.Module);
                i.detail = 'Auth block — OAuth2 Authorization Code with PKCE';
                i.insertText = new vscode.SnippetString('auth ${1:my_auth}(auth_type.oauth2_authorization_code) {\n\tclient_id: "${2:}",\n\tauthorization_url: "${3:}",\n\ttoken_url: "${4:}",\n\tredirect_uri: "${5:}",\n\tscope: "${6:}"\n}');
                i.sortText = 'auth_2oauth_ac';
                return i;
            })(),
            (() => {
                const i = new vscode.CompletionItem('auth oauth2_client_credentials (cert_file)', vscode.CompletionItemKind.Module);
                i.detail = 'Auth block — OAuth2 Client Credentials (certificate)';
                i.insertText = new vscode.SnippetString('auth ${1:my_auth}(auth_type.oauth2_client_credentials) {\n\tclient_id: "${2:}",\n\tcert_file: "${3:}",\n\tcert_password: "${4:}",\n\ttoken_url: "${5:}",\n\tscope: "${6:}"\n}');
                i.sortText = 'auth_2oauth_cc_cert';
                return i;
            })(),
            (() => {
                const i = new vscode.CompletionItem('auth oauth2_client_credentials (client_secret)', vscode.CompletionItemKind.Module);
                i.detail = 'Auth block — OAuth2 Client Credentials (client secret)';
                i.insertText = new vscode.SnippetString('auth ${1:my_auth}(auth_type.oauth2_client_credentials) {\n\tclient_id: "${2:}",\n\tclient_secret: "${3:}",\n\ttoken_url: "${4:}",\n\tscope: "${5:}"\n}');
                i.sortText = 'auth_2oauth_cc_secret';
                return i;
            })(),
            (() => {
                const i = new vscode.CompletionItem('auth oauth2_implicit', vscode.CompletionItemKind.Module);
                i.detail = 'Auth block — OAuth2 Implicit Flow';
                i.insertText = new vscode.SnippetString('auth ${1:my_auth}(auth_type.oauth2_implicit) {\n\tclient_id: "${2:}",\n\tauthorization_url: "${3:}",\n\tscope: "${4:}"\n}');
                i.sortText = 'auth_2oauth_impl';
                return i;
            })(),
            kw('ep', 'Endpoint block keyword', 'ep_0kw'),
            (() => {
                const i = new vscode.CompletionItem('ep …', vscode.CompletionItemKind.Module);
                i.detail = 'Endpoint block snippet';
                i.insertText = new vscode.SnippetString('ep ${1:ep_name}($0) {\n}');
                i.sortText = 'ep_1sn';
                return i;
            })(),
            (() => {
                const i = new vscode.CompletionItem('ep crud', vscode.CompletionItemKind.Module);
                i.detail = 'CRUD endpoint snippet';
                i.insertText = new vscode.SnippetString(
                    'let ${1:endpoint}_id = "";\n\nep ${1:endpoint}s($0) {\n\trq list();\n\trq get();\n\trq post(body: io.read_file("${1:endpoint}-post.json"));\n\trq patch(url: ${1:endpoint}_id, body: io.read_file("${1:endpoint}-patch.json"));\n\trq delete();\n}'
                );
                i.sortText = 'ep_2crud';
                return i;
            })(),
            kw('env', 'Environment block keyword', 'env_0kw'),
            (() => {
                const i = new vscode.CompletionItem('env …', vscode.CompletionItemKind.Module);
                i.detail = 'Environment block snippet';
                i.insertText = new vscode.SnippetString('env ${1:local} {\n\t${2:api_url}: "${3:http://localhost:8080}"\n}');
                i.sortText = 'env_1sn';
                return i;
            })(),
            ...(!hasNonImportContent ? [kw('import', 'Import directive keyword', 'import_0kw')] : []),
            kw('let', 'Variable declaration keyword', 'let_0kw'),
            kw('rq', 'HTTP request keyword', 'rq_0kw'),
            (() => {
                const i = new vscode.CompletionItem('rq …', vscode.CompletionItemKind.Module);
                i.detail = 'HTTP request snippet';
                i.insertText = new vscode.SnippetString('rq ${1:rq_name}($0);');
                i.sortText = 'rq_1sn';
                return i;
            })(),
        ];
    },
};

export const ALL_HANDLERS: CompletionHandler[] = [
    epTemplateHandler,
    importHandler,
    dollarPrefixHandler,
    letAssignmentHandler,
    interpolationHandler,
    codeChallengeMethodHandler,
    propertyValueHandler,
    authPropertyNameHandler,
    authTypeHandler,
    authDeclarationHandler,
    rqBlockHandler,
    epBlockHandler,
    authNameValueHandler,
    attributeHandler,
    headerKeyHandler,
    namespaceHandler,
    topLevelKeywordHandler,
];
