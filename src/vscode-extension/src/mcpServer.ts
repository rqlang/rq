import * as vscode from 'vscode';
import * as fs from 'fs';
import * as path from 'path';

const PROVIDER_ID = 'rq-lang.rq-mcp';
const SERVER_LABEL = 'rq';
const SERVER_ENTRY = ['out', 'mcpServer.js'];

export function mcpServerEntry(extensionPath: string): string {
    return path.join(extensionPath, ...SERVER_ENTRY);
}

export function registerMcpServer(context: vscode.ExtensionContext): void {
    const entry = mcpServerEntry(context.extensionPath);
    if (!fs.existsSync(entry)) {
        console.error(`rq MCP server entry missing at ${entry}; skipping registration`);
        return;
    }

    const version = context.extension?.packageJSON?.version;
    const changeEmitter = new vscode.EventEmitter<void>();
    context.subscriptions.push(changeEmitter);
    context.subscriptions.push(vscode.workspace.onDidChangeWorkspaceFolders(() => changeEmitter.fire()));

    context.subscriptions.push(vscode.lm.registerMcpServerDefinitionProvider(PROVIDER_ID, {
        onDidChangeMcpServerDefinitions: changeEmitter.event,
        provideMcpServerDefinitions: () => {
            const definition = new vscode.McpStdioServerDefinition(
                SERVER_LABEL,
                process.execPath,
                [entry],
                { RQ_MCP_VERSION: version ?? '', ELECTRON_RUN_AS_NODE: '1' },
                version
            );
            const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
            if (workspaceFolder) {
                definition.cwd = workspaceFolder.uri;
            }
            return [definition];
        }
    }));
}
