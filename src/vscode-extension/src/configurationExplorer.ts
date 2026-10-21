import * as vscode from 'vscode';
import * as rqClient from './rqClient';
import { applyTreeItemLoading } from './vscodeUtils';
import { ExplorerState, RestorableTreeDataProvider, TreeItemIds } from './explorerState';

type SectionKind = 'section-environments' | 'section-auth';
type EntryKind = 'environment' | 'auth-config';
type ItemKind = SectionKind | EntryKind | 'loading';

interface SectionEntry {
    name: string;
    description?: string;
    tooltip: string;
}

interface SectionDefinition {
    kind: SectionKind;
    label: string;
    entryKind: EntryKind;
    entryIcon: string;
    artifactType: 'env' | 'auth';
    errorLabel: string;
    list: (workspaceRoot: string | undefined) => Promise<{ entries: SectionEntry[]; errors: string[] }>;
}

const SECTIONS: SectionDefinition[] = [
    {
        kind: 'section-environments',
        label: 'Environments',
        entryKind: 'environment',
        entryIcon: 'server-environment',
        artifactType: 'env',
        errorLabel: 'environments',
        list: async root => {
            const { environments, errors } = await rqClient.listEnvironmentsWithErrors(root);
            return { entries: environments.map(name => ({ name, tooltip: name })), errors };
        },
    },
    {
        kind: 'section-auth',
        label: 'Auth',
        entryKind: 'auth-config',
        entryIcon: 'key',
        artifactType: 'auth',
        errorLabel: 'auth configs',
        list: async root => {
            const { authConfigs, errors } = await rqClient.listAuthConfigsWithErrors(root);
            const entries = authConfigs.map(e => ({ name: e.name, description: e.auth_type, tooltip: `${e.name} (${e.auth_type})` }));
            return { entries, errors };
        },
    },
];

export class ConfigurationExplorerProvider implements RestorableTreeDataProvider<ConfigurationTreeItem> {
    private _onDidChangeTreeData = new vscode.EventEmitter<ConfigurationTreeItem | undefined | null | void>();
    readonly onDidChangeTreeData = this._onDidChangeTreeData.event;
    private _onDidLoad = new vscode.EventEmitter<void>();
    readonly onDidLoad = this._onDidLoad.event;
    private readonly originalIcons = new WeakMap<ConfigurationTreeItem, vscode.TreeItem['iconPath']>();
    private readonly sectionItems = new Map<SectionKind, ConfigurationTreeItem>();
    private readonly cachedChildren = new Map<SectionKind, ConfigurationTreeItem[]>();
    private readonly incompleteSections = new Set<SectionKind>();
    private readonly loadingSections = new Set<SectionKind>();
    private readonly reloadPending = new Set<SectionKind>();
    private readonly notifyPending = new Set<SectionKind>();

    constructor(private workspaceRoot: string | undefined, private readonly state: ExplorerState) {}

    refresh(): void {
        if (this.cachedChildren.size === 0 && this.loadingSections.size === 0) {
            this._onDidChangeTreeData.fire();
            return;
        }
        this.reload(true);
    }

    refreshInBackground(): void {
        this.reload(false);
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
        const section = SECTIONS.find(({ entryKind }) => id.startsWith(`${entryKind}:`));
        if (!section) {
            return true;
        }
        return this.cachedChildren.has(section.kind) && !this.incompleteSections.has(section.kind);
    }

    getParent(element: ConfigurationTreeItem): ConfigurationTreeItem | undefined {
        return element.section ? this.sectionItems.get(element.section) : undefined;
    }

    async getChildren(element?: ConfigurationTreeItem): Promise<ConfigurationTreeItem[]> {
        if (!this.workspaceRoot) {
            return [];
        }

        if (!element) {
            const sections = SECTIONS.map(section => this.getSectionItem(section));
            setTimeout(() => this._onDidLoad.fire(), 0);
            return sections;
        }

        const section = SECTIONS.find(({ kind }) => kind === element.kind);
        if (!section) {
            return [];
        }
        const cached = this.cachedChildren.get(section.kind);
        if (cached) {
            return cached;
        }
        if (!this.loadingSections.has(section.kind)) {
            void this.loadSection(section, true);
        }
        return [this.makeLoadingItem(section.kind)];
    }

    private reload(notifyErrors: boolean): void {
        for (const section of SECTIONS) {
            if (this.loadingSections.has(section.kind)) {
                this.reloadPending.add(section.kind);
                if (notifyErrors) { this.notifyPending.add(section.kind); }
            } else if (this.cachedChildren.has(section.kind)) {
                void this.loadSection(section, notifyErrors);
            }
        }
    }

    private getSectionItem(section: SectionDefinition): ConfigurationTreeItem {
        const collapsibleState = this.state.collapsibleState(section.kind, vscode.TreeItemCollapsibleState.Collapsed);
        const existing = this.sectionItems.get(section.kind);
        if (existing) {
            existing.collapsibleState = collapsibleState;
            return existing;
        }
        const item = new ConfigurationTreeItem(section.label, section.kind, collapsibleState);
        item.id = section.kind;
        this.sectionItems.set(section.kind, item);
        return item;
    }

    private makeLoadingItem(section: SectionKind): ConfigurationTreeItem {
        const item = new ConfigurationTreeItem('Loading…', 'loading', vscode.TreeItemCollapsibleState.None, section);
        item.id = `loading:${section}`;
        item.iconPath = new vscode.ThemeIcon('sync~spin');
        return item;
    }

    private async loadSection(section: SectionDefinition, notifyErrors: boolean): Promise<void> {
        const progressItem = notifyErrors && this.cachedChildren.has(section.kind) ? this.sectionItems.get(section.kind) : undefined;
        if (progressItem) { this.setItemLoading(progressItem, true); }
        this.loadingSections.add(section.kind);
        do {
            this.reloadPending.delete(section.kind);
            const { items, complete } = await this.loadSectionItems(section, notifyErrors);
            this.cachedChildren.set(section.kind, items);
            if (complete) {
                this.incompleteSections.delete(section.kind);
            } else {
                this.incompleteSections.add(section.kind);
            }
            notifyErrors = this.notifyPending.delete(section.kind);
        } while (this.reloadPending.has(section.kind));
        this.loadingSections.delete(section.kind);
        if (progressItem) { this.setItemLoading(progressItem, false); }
        this._onDidChangeTreeData.fire(this.sectionItems.get(section.kind));
        this._onDidLoad.fire();
    }

    private async loadSectionItems(section: SectionDefinition, notifyErrors: boolean): Promise<{ items: ConfigurationTreeItem[]; complete: boolean }> {
        try {
            const { entries, errors } = await section.list(this.workspaceRoot);
            const ids = new TreeItemIds();
            const items = entries.map(entry => {
                const item = new ConfigurationTreeItem(entry.name, section.entryKind, vscode.TreeItemCollapsibleState.None, section.kind);
                item.id = ids.next(`${section.entryKind}:${entry.name}`);
                item.iconPath = new vscode.ThemeIcon(section.entryIcon);
                item.description = entry.description;
                item.tooltip = entry.tooltip;
                item.command = { command: 'rq.openConfigurationFile', title: 'Open File', arguments: [section.artifactType, entry.name, item] };
                return item;
            });
            return { items, complete: errors.length === 0 };
        } catch (error) {
            if (notifyErrors) {
                vscode.window.showErrorMessage(`Failed to load ${section.errorLabel}: ${error instanceof Error ? error.message : 'Unknown error'}`);
            }
            return { items: [], complete: false };
        }
    }
}

export class ConfigurationTreeItem extends vscode.TreeItem {
    constructor(
        public readonly label: string,
        public readonly kind: ItemKind,
        public collapsibleState: vscode.TreeItemCollapsibleState,
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
