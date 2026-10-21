import * as vscode from 'vscode';
import * as rqClient from './rqClient';
import { applyTreeItemLoading } from './vscodeUtils';
import { ExplorerState, RestorableTreeDataProvider, TreeItemIds } from './explorerState';

type SectionKind = 'section-environments' | 'section-auth';
type ItemKind = SectionKind | 'environment' | 'auth-config' | 'loading';

const SECTIONS: { kind: SectionKind; label: string; itemKind: ItemKind }[] = [
    { kind: 'section-environments', label: 'Environments', itemKind: 'environment' },
    { kind: 'section-auth', label: 'Auth', itemKind: 'auth-config' },
];

export class ConfigurationExplorerProvider implements RestorableTreeDataProvider<ConfigurationTreeItem> {
    private _onDidChangeTreeData = new vscode.EventEmitter<ConfigurationTreeItem | undefined | null | void>();
    readonly onDidChangeTreeData = this._onDidChangeTreeData.event;
    private _onDidLoad = new vscode.EventEmitter<void>();
    readonly onDidLoad = this._onDidLoad.event;
    private readonly originalIcons = new WeakMap<ConfigurationTreeItem, vscode.TreeItem['iconPath']>();
    private readonly sectionItems = new Map<SectionKind, ConfigurationTreeItem>();
    private readonly cachedChildren = new Map<SectionKind, ConfigurationTreeItem[]>();
    private readonly loadingSections = new Set<SectionKind>();
    private readonly reloadPending = new Set<SectionKind>();

    constructor(private workspaceRoot: string | undefined, private readonly state: ExplorerState) {}

    refresh(): void {
        if (this.cachedChildren.size === 0 && this.loadingSections.size === 0) {
            this._onDidChangeTreeData.fire();
            return;
        }
        for (const { kind } of SECTIONS) {
            if (this.loadingSections.has(kind)) {
                this.reloadPending.add(kind);
            } else if (this.cachedChildren.has(kind)) {
                void this.loadSection(kind);
            }
        }
    }

    setItemLoading(item: ConfigurationTreeItem, loading: boolean): void {
        applyTreeItemLoading(item, loading, this.originalIcons, (i) => this._onDidChangeTreeData.fire(i as ConfigurationTreeItem));
    }

    getTreeItem(element: ConfigurationTreeItem): vscode.TreeItem {
        return element;
    }

    findItem(id: string): ConfigurationTreeItem | undefined {
        const sections = [...this.sectionItems.values()];
        const children = [...this.cachedChildren.values()].flat();
        return [...sections, ...children].find(item => item.id === id);
    }

    isLoaded(id: string): boolean {
        const section = SECTIONS.find(({ itemKind }) => id.startsWith(`${itemKind}:`));
        return section ? this.cachedChildren.has(section.kind) : true;
    }

    getParent(element: ConfigurationTreeItem): ConfigurationTreeItem | undefined {
        return element.section ? this.sectionItems.get(element.section) : undefined;
    }

    async getChildren(element?: ConfigurationTreeItem): Promise<ConfigurationTreeItem[]> {
        if (!this.workspaceRoot) {
            return [];
        }

        if (!element) {
            return SECTIONS.map(({ kind, label }) => this.makeSectionItem(kind, label));
        }

        if (element.kind === 'section-environments' || element.kind === 'section-auth') {
            const cached = this.cachedChildren.get(element.kind);
            if (cached) {
                return cached;
            }
            if (!this.loadingSections.has(element.kind)) {
                void this.loadSection(element.kind);
            }
            return [this.makeLoadingItem(element.kind)];
        }

        return [];
    }

    private makeSectionItem(kind: SectionKind, label: string): ConfigurationTreeItem {
        const item = new ConfigurationTreeItem(label, kind, this.state.collapsibleState(kind, vscode.TreeItemCollapsibleState.Collapsed));
        item.id = kind;
        this.sectionItems.set(kind, item);
        return item;
    }

    private makeLoadingItem(section: SectionKind): ConfigurationTreeItem {
        const item = new ConfigurationTreeItem('Loading…', 'loading', vscode.TreeItemCollapsibleState.None, section);
        item.id = `loading:${section}`;
        item.iconPath = new vscode.ThemeIcon('sync~spin');
        return item;
    }

    private async loadSection(kind: SectionKind): Promise<void> {
        this.loadingSections.add(kind);
        do {
            this.reloadPending.delete(kind);
            const items = kind === 'section-environments' ? await this.loadEnvironments() : await this.loadAuthConfigs();
            this.cachedChildren.set(kind, items);
        } while (this.reloadPending.has(kind));
        this.loadingSections.delete(kind);
        this._onDidChangeTreeData.fire(this.sectionItems.get(kind));
        this._onDidLoad.fire();
    }

    private async loadEnvironments(): Promise<ConfigurationTreeItem[]> {
        try {
            const names = await rqClient.listEnvironments(this.workspaceRoot);
            const ids = new TreeItemIds();
            return names.map(name => {
                const item = new ConfigurationTreeItem(name, 'environment', vscode.TreeItemCollapsibleState.None, 'section-environments');
                item.id = ids.next(`environment:${name}`);
                item.iconPath = new vscode.ThemeIcon('server-environment');
                item.tooltip = name;
                item.command = { command: 'rq.openConfigurationFile', title: 'Open File', arguments: ['env', name, item] };
                return item;
            });
        } catch (error) {
            vscode.window.showErrorMessage(`Failed to load environments: ${error instanceof Error ? error.message : 'Unknown error'}`);
            return [];
        }
    }

    private async loadAuthConfigs(): Promise<ConfigurationTreeItem[]> {
        try {
            const entries = await rqClient.listAuthConfigs(this.workspaceRoot);
            const ids = new TreeItemIds();
            return entries.map(e => {
                const item = new ConfigurationTreeItem(e.name, 'auth-config', vscode.TreeItemCollapsibleState.None, 'section-auth');
                item.id = ids.next(`auth-config:${e.name}`);
                item.iconPath = new vscode.ThemeIcon('key');
                item.description = e.auth_type;
                item.tooltip = `${e.name} (${e.auth_type})`;
                item.command = { command: 'rq.openConfigurationFile', title: 'Open File', arguments: ['auth', e.name, item] };
                return item;
            });
        } catch (error) {
            vscode.window.showErrorMessage(`Failed to load auth configs: ${error instanceof Error ? error.message : 'Unknown error'}`);
            return [];
        }
    }
}

export class ConfigurationTreeItem extends vscode.TreeItem {
    constructor(
        public readonly label: string,
        public readonly kind: ItemKind,
        public readonly collapsibleState: vscode.TreeItemCollapsibleState,
        public readonly section?: SectionKind,
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
