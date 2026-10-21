import * as vscode from 'vscode';

interface StoredExplorerState {
    expanded?: Record<string, boolean>;
    selected?: string;
    environment?: string;
}

export interface RestorableTreeDataProvider<T extends vscode.TreeItem> extends vscode.TreeDataProvider<T> {
    readonly onDidLoad: vscode.Event<void>;
    findItem(id: string): T | undefined;
    isLoaded(id: string): boolean;
    getParent(element: T): T | undefined;
}

export class ExplorerState {
    constructor(private readonly memento: vscode.Memento, private readonly key: string) {}

    get environment(): string | undefined {
        return this.read().environment;
    }

    get selected(): string | undefined {
        return this.read().selected;
    }

    collapsibleState(id: string, fallback: vscode.TreeItemCollapsibleState): vscode.TreeItemCollapsibleState {
        const expanded = this.read().expanded?.[id];
        if (expanded === undefined) {
            return fallback;
        }
        return expanded ? vscode.TreeItemCollapsibleState.Expanded : vscode.TreeItemCollapsibleState.Collapsed;
    }

    setEnvironment(environment: string | undefined): void {
        this.write({ environment });
    }

    setSelected(id: string | undefined): void {
        this.write({ selected: id });
    }

    setExpanded(id: string, expanded: boolean): void {
        this.write({ expanded: { ...this.read().expanded, [id]: expanded } });
    }

    private read(): StoredExplorerState {
        return this.memento.get<StoredExplorerState>(this.key) ?? {};
    }

    private write(changes: Partial<StoredExplorerState>): void {
        this.memento.update(this.key, { ...this.read(), ...changes })
            .then(undefined, err => console.error('Failed to store explorer state', err));
    }
}

export class TreeItemIds {
    private readonly used = new Set<string>();

    next(base: string): string {
        let id = base;
        for (let suffix = 2; this.used.has(id); suffix++) {
            id = `${base}~${suffix}`;
        }
        this.used.add(id);
        return id;
    }
}

export function trackExplorerState<T extends vscode.TreeItem>(
    view: vscode.TreeView<T>,
    provider: RestorableTreeDataProvider<T>,
    state: ExplorerState
): vscode.Disposable[] {
    let pendingSelection = state.selected;
    return [
        view.onDidExpandElement(({ element }) => {
            if (element.id) { state.setExpanded(element.id, true); }
        }),
        view.onDidCollapseElement(({ element }) => {
            if (element.id) { state.setExpanded(element.id, false); }
        }),
        view.onDidChangeSelection(({ selection }) => {
            const previous = state.selected;
            if (selection.length === 0 && previous && !provider.findItem(previous) && !provider.isLoaded(previous)) {
                pendingSelection = previous;
                return;
            }
            pendingSelection = undefined;
            state.setSelected(selection[0]?.id);
        }),
        provider.onDidLoad(() => {
            if (!pendingSelection || !view.visible) { return; }
            const item = provider.findItem(pendingSelection);
            if (!item) {
                if (provider.isLoaded(pendingSelection)) {
                    pendingSelection = undefined;
                    state.setSelected(undefined);
                }
                return;
            }
            pendingSelection = undefined;
            if (!ancestorsExpanded(provider, state, item)) { return; }
            view.reveal(item, { select: true, focus: false })
                .then(undefined, err => console.error('Failed to restore explorer selection', err));
        })
    ];
}

function ancestorsExpanded<T extends vscode.TreeItem>(provider: RestorableTreeDataProvider<T>, state: ExplorerState, item: T): boolean {
    for (let parent = provider.getParent(item); parent; parent = provider.getParent(parent)) {
        const collapsibleState = parent.id
            ? state.collapsibleState(parent.id, parent.collapsibleState ?? vscode.TreeItemCollapsibleState.None)
            : parent.collapsibleState;
        if (collapsibleState !== vscode.TreeItemCollapsibleState.Expanded) {
            return false;
        }
    }
    return true;
}
