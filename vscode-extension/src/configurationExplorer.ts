import * as vscode from 'vscode';
import * as cliService from './cliService';

type ItemKind = 'section-environments' | 'section-auth' | 'environment' | 'auth-config';

export class ConfigurationExplorerProvider implements vscode.TreeDataProvider<ConfigurationTreeItem> {
    private _onDidChangeTreeData = new vscode.EventEmitter<ConfigurationTreeItem | undefined | null | void>();
    readonly onDidChangeTreeData = this._onDidChangeTreeData.event;

    constructor(private workspaceRoot: string | undefined) {}

    refresh(): void {
        this._onDidChangeTreeData.fire();
    }

    getTreeItem(element: ConfigurationTreeItem): vscode.TreeItem {
        return element;
    }

    async getChildren(element?: ConfigurationTreeItem): Promise<ConfigurationTreeItem[]> {
        if (!this.workspaceRoot) {
            return [];
        }

        if (!element) {
            return [
                new ConfigurationTreeItem('Environments', 'section-environments', vscode.TreeItemCollapsibleState.Collapsed),
                new ConfigurationTreeItem('Auth', 'section-auth', vscode.TreeItemCollapsibleState.Collapsed),
            ];
        }

        try {
            if (element.kind === 'section-environments') {
                const names = await cliService.listEnvironments(this.workspaceRoot);
                return names.map(name => {
                    const item = new ConfigurationTreeItem(name, 'environment', vscode.TreeItemCollapsibleState.None);
                    item.iconPath = new vscode.ThemeIcon('server-environment');
                    item.command = { command: 'rq.openConfigurationFile', title: 'Open File', arguments: ['env', name] };
                    return item;
                });
            }

            if (element.kind === 'section-auth') {
                const entries = await cliService.listAuthConfigs(this.workspaceRoot);
                return entries.map(e => {
                    const item = new ConfigurationTreeItem(e.name, 'auth-config', vscode.TreeItemCollapsibleState.None);
                    item.iconPath = new vscode.ThemeIcon('key');
                    item.description = e.auth_type;
                    item.command = { command: 'rq.openConfigurationFile', title: 'Open File', arguments: ['auth', e.name] };
                    return item;
                });
            }
        } catch (error) {
            vscode.window.showErrorMessage(`Failed to load configuration: ${error instanceof Error ? error.message : 'Unknown error'}`);
        }

        return [];
    }
}

export class ConfigurationTreeItem extends vscode.TreeItem {
    constructor(
        public readonly label: string,
        public readonly kind: ItemKind,
        public readonly collapsibleState: vscode.TreeItemCollapsibleState,
    ) {
        super(label, collapsibleState);
        this.contextValue = kind;

        if (kind === 'section-environments') {
            this.iconPath = new vscode.ThemeIcon('server-environment');
        } else if (kind === 'section-auth') {
            this.iconPath = new vscode.ThemeIcon('lock');
        }
    }
}
