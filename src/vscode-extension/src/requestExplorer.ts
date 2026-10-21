import * as vscode from 'vscode';
import * as rqClient from './rqClient';
import * as path from 'path';
import { normalizePath } from './utils';
import { applyTreeItemLoading } from './vscodeUtils';
import { ExplorerState, RestorableTreeDataProvider, TreeItemIds } from './explorerState';
import { UI_COLORS, UiColor } from './language/definitions';

export const ENVIRONMENT_COLOR_SCHEME = 'rq-environment';

export const environmentColorDecorationProvider: vscode.FileDecorationProvider = {
    provideFileDecoration(uri: vscode.Uri): vscode.FileDecoration | undefined {
        if (uri.scheme !== ENVIRONMENT_COLOR_SCHEME || !UI_COLORS.includes(uri.query as UiColor)) {
            return undefined;
        }
        return new vscode.FileDecoration(undefined, undefined, environmentThemeColor(uri.query as UiColor));
    }
};

function environmentThemeColor(color: UiColor): vscode.ThemeColor {
    return new vscode.ThemeColor(`rq.environment.${color}`);
}

function environmentLabel(value: string): string {
    return `Environment: ${value}`;
}

export interface RequestInfo {
    name: string;
    endpoint: string | null;
    file: string;
    endpoint_file?: string;
    endpoint_line?: number;
    endpoint_character?: number;
}



/**
 * Tree data provider for RQ requests explorer
 */
export class RequestExplorerProvider implements RestorableTreeDataProvider<RequestTreeItem> {
    private _onDidChangeTreeData: vscode.EventEmitter<RequestTreeItem | undefined | null | void> = new vscode.EventEmitter<RequestTreeItem | undefined | null | void>();
    readonly onDidChangeTreeData: vscode.Event<RequestTreeItem | undefined | null | void> = this._onDidChangeTreeData.event;
    private _onDidChangeEnvironment: vscode.EventEmitter<string | undefined> = new vscode.EventEmitter<string | undefined>();
    readonly onDidChangeEnvironment: vscode.Event<string | undefined> = this._onDidChangeEnvironment.event;
    private _onDidLoad = new vscode.EventEmitter<void>();
    readonly onDidLoad: vscode.Event<void> = this._onDidLoad.event;
    private selectedEnvironment: string | undefined;
    private missingEnvironment: string | undefined;
    private environmentColors: Record<string, UiColor> = {};
    private envItem: RequestTreeItem | undefined;
    private readonly originalIcons = new WeakMap<RequestTreeItem, vscode.TreeItem['iconPath']>();
    private cachedRootItems: RequestTreeItem[] | null = null;
    private rootItemsComplete = false;
    private loadingRootItems = false;
    private reloadPending = false;
    private notifyPending = false;

    constructor(private workspaceRoot: string | undefined, private readonly state: ExplorerState) {
        this.selectedEnvironment = state.environment;
    }

    setTreeLoading(loading: boolean): void {
        if (!this.envItem) { return; }
        if (loading) {
            this.envItem.iconPath = new vscode.ThemeIcon('sync~spin');
            this.envItem.label = environmentLabel('Loading\u2026');
            this.envItem.resourceUri = undefined;
        } else {
            this.describeEnvironment(this.envItem);
        }
        this._onDidChangeTreeData.fire(this.envItem);
    }

    setItemLoading(item: RequestTreeItem, loading: boolean): void {
        applyTreeItemLoading(item, loading, this.originalIcons, (i) => this._onDidChangeTreeData.fire(i as RequestTreeItem));
    }

    setItemRunning(item: RequestTreeItem, running: boolean): void {
        item.contextValue = running ? 'request-running' : 'request';
        this.setItemLoading(item, running);
    }

    getSelectedEnvironment(): string | undefined {
        return this.selectedEnvironment;
    }

    setSelectedEnvironment(environment: string | undefined): void {
        this.selectedEnvironment = environment;
        this.missingEnvironment = undefined;
        this.state.setEnvironment(environment);
        this._onDidChangeEnvironment.fire(environment);
        if (this.envItem) {
            this.describeEnvironment(this.envItem);
            this._onDidChangeTreeData.fire(this.envItem);
        }
    }

    refresh(): void {
        this.reload(true);
    }

    refreshInBackground(): void {
        if (this.cachedRootItems === null && !this.loadingRootItems) {
            return;
        }
        this.reload(false);
    }

    async warnIfEnvironmentMissing(): Promise<void> {
        if (!this.selectedEnvironment) {
            return;
        }
        const { missingEnvironment, colors } = await this.inspectEnvironments();
        this.environmentColors = colors;
        if (!missingEnvironment || missingEnvironment !== this.selectedEnvironment) {
            return;
        }
        this.missingEnvironment = missingEnvironment;
        if (this.envItem) {
            this.describeEnvironment(this.envItem);
            this._onDidChangeTreeData.fire(this.envItem);
        }
        vscode.window.showWarningMessage(`Environment '${missingEnvironment}' no longer exists. Select another one in the Requests view.`);
    }

    findItem(id: string): RequestTreeItem | undefined {
        if (this.envItem?.id === id) {
            return this.envItem;
        }
        return findItemById(this.cachedRootItems ?? [], id);
    }

    isLoaded(): boolean {
        return this.rootItemsComplete;
    }

    getParent(element: RequestTreeItem): RequestTreeItem | undefined {
        return element.parent;
    }

    getTreeItem(element: RequestTreeItem): vscode.TreeItem {
        return element;
    }

    getChildren(element?: RequestTreeItem): RequestTreeItem[] | Promise<RequestTreeItem[]> {
        if (!this.workspaceRoot) {
            vscode.window.showInformationMessage('No workspace folder open');
            return [];
        }

        if (!element) {
            const envItem = this.buildEnvInfoItem();
            if (this.cachedRootItems !== null) {
                return [envItem, ...this.cachedRootItems];
            }
            if (!this.loadingRootItems) {
                void this.loadRootItems(true);
            }
            return [envItem, this.makeLoadingItem()];
        }

        return element.children || [];
    }

    private reload(notifyErrors: boolean): void {
        if (this.loadingRootItems) {
            this.reloadPending = true;
            this.notifyPending ||= notifyErrors;
            return;
        }
        if (this.cachedRootItems === null) {
            this._onDidChangeTreeData.fire();
            return;
        }
        if (notifyErrors) {
            this.setTreeLoading(true);
        }
        void this.loadRootItems(notifyErrors);
    }

    private buildEnvInfoItem(): RequestTreeItem {
        const item = new RequestTreeItem(environmentLabel('None'), null, vscode.TreeItemCollapsibleState.None);
        item.id = 'environment';
        item.contextValue = 'environment-info';
        this.describeEnvironment(item);
        this.envItem = item;
        return item;
    }

    private describeEnvironment(item: RequestTreeItem): void {
        const environment = this.selectedEnvironment;
        if (environment && environment === this.missingEnvironment) {
            item.resourceUri = undefined;
            item.iconPath = new vscode.ThemeIcon('warning');
            item.label = environmentLabel(`${environment} (not found)`);
            item.tooltip = `Environment '${environment}' no longer exists.\n\nClick the environment icon in the toolbar to select another one.`;
            return;
        }
        const color = environment ? this.environmentColors[environment] : undefined;
        item.resourceUri = environment && color
            ? vscode.Uri.from({ scheme: ENVIRONMENT_COLOR_SCHEME, path: `/${environment}`, query: color })
            : undefined;
        item.iconPath = environment
            ? new vscode.ThemeIcon('server-environment', color ? environmentThemeColor(color) : undefined)
            : new vscode.ThemeIcon('circle-slash');
        item.label = environmentLabel(environment || 'None');
        item.tooltip = environment
            ? `Current environment: ${environment}\n\nClick the environment icon in the toolbar to change.`
            : 'No environment selected.\n\nClick the environment icon in the toolbar to select one.';
    }

    private makeLoadingItem(): RequestTreeItem {
        const item = new RequestTreeItem('Loading…', null, vscode.TreeItemCollapsibleState.None);
        item.id = 'loading';
        item.contextValue = 'loading';
        item.iconPath = new vscode.ThemeIcon('sync~spin');
        return item;
    }

    private async loadRootItems(notifyErrors: boolean): Promise<void> {
        this.loadingRootItems = true;
        do {
            this.reloadPending = false;
            const [root, environments] = await Promise.all([this.getRootItems(notifyErrors), this.inspectEnvironments()]);
            this.cachedRootItems = root.items;
            this.rootItemsComplete = root.complete;
            this.missingEnvironment = environments.missingEnvironment;
            this.environmentColors = environments.colors;
            notifyErrors = this.notifyPending;
            this.notifyPending = false;
        } while (this.reloadPending);
        this.loadingRootItems = false;
        this._onDidChangeTreeData.fire();
        this._onDidLoad.fire();
    }

    private async inspectEnvironments(): Promise<{ missingEnvironment: string | undefined; colors: Record<string, UiColor> }> {
        const environment = this.selectedEnvironment;
        try {
            const { environments, colors, errors } = await rqClient.listEnvironmentsWithErrors(this.workspaceRoot);
            const missing = environment !== undefined && errors.length === 0 && !environments.includes(environment);
            return { missingEnvironment: missing ? environment : undefined, colors };
        } catch {
            return { missingEnvironment: undefined, colors: this.environmentColors };
        }
    }

    private async getRootItems(notifyErrors: boolean): Promise<{ items: RequestTreeItem[]; complete: boolean }> {
        try {
            const result = await rqClient.listRequests(this.workspaceRoot);
            const requestItems = this.groupRequestsByFolder(result.requests);

            if (result.errors && result.errors.length > 0) {
                const errorItem = new RequestTreeItem(
                    `Parse Errors (${result.errors.length})`,
                    null,
                    vscode.TreeItemCollapsibleState.None
                );
                errorItem.id = 'parse-errors';
                errorItem.contextValue = 'error';
                errorItem.iconPath = new vscode.ThemeIcon('error');
                errorItem.tooltip = result.errors.join('\n');
                errorItem.description = 'Check Output panel';
                return { items: [errorItem, ...requestItems], complete: false };
            }

            return { items: requestItems, complete: true };
        } catch (error) {
            const errorMessage = error instanceof Error ? error.message : 'Unknown error';
            if (notifyErrors) {
                vscode.window.showErrorMessage(`Failed to list requests: ${errorMessage}`);
            }

            const errorItem = new RequestTreeItem(
                'Error loading requests',
                null,
                vscode.TreeItemCollapsibleState.None
            );
            errorItem.id = 'load-error';
            errorItem.contextValue = 'error';
            errorItem.iconPath = new vscode.ThemeIcon('error');
            errorItem.tooltip = errorMessage;
            errorItem.description = 'Check Output panel for details';
            return { items: [errorItem], complete: false };
        }
    }

    private groupRequestsByFolder(requests: RequestInfo[]): RequestTreeItem[] {
        if (!this.workspaceRoot) {
            return [];
        }

        // Normalize workspace root to handle drive letter casing issues on Windows
        const normalizedRoot = normalizePath(this.workspaceRoot);
        const ids = new TreeItemIds();

        // Build folder hierarchy
        interface FolderNode {
            name: string;
            fullPath: string;
            requests: RequestInfo[];
            subfolders: Map<string, FolderNode>;
        }

        const rootNode: FolderNode = {
            name: '',
            fullPath: normalizedRoot,
            requests: [],
            subfolders: new Map()
        };

        // Group requests by their folder paths
        for (const request of requests) {
            // Normalize request file path
            const normalizedFile = normalizePath(request.file);
            const relativePath = path.relative(normalizedRoot, path.dirname(normalizedFile));
            
            // If file is in workspace root, add to root requests
            if (!relativePath || relativePath === '.') {
                rootNode.requests.push(request);
                continue;
            }

            // If path is outside workspace (starts with ..), treat as root to avoid UI issues
            if (relativePath.startsWith('..')) {
                rootNode.requests.push(request);
                continue;
            }

            // Split path into folder components
            const folders = relativePath.split(path.sep);
            let currentNode = rootNode;

            // Navigate/create folder hierarchy
            for (const folder of folders) {
                if (!currentNode.subfolders.has(folder)) {
                    currentNode.subfolders.set(folder, {
                        name: folder,
                        fullPath: path.join(currentNode.fullPath, folder),
                        requests: [],
                        subfolders: new Map()
                    });
                }
                currentNode = currentNode.subfolders.get(folder)!;
            }

            // Add request to the deepest folder
            currentNode.requests.push(request);
        }

        // Convert folder tree to RequestTreeItem hierarchy
        const convertNodeToItems = (node: FolderNode, isRoot: boolean = false): RequestTreeItem[] => {
            const items: RequestTreeItem[] = [];

            // Add subfolders
            for (const [folderName, subNode] of node.subfolders.entries()) {
                // First add nested subfolders
                const folderChildren = convertNodeToItems(subNode, false);
                
                // Then add requests in this subfolder grouped by endpoint
                const folderPath = path.relative(normalizedRoot, subNode.fullPath);
                const requestsByEndpoint = this.groupRequestsByEndpointInFolder(subNode.requests, folderPath, ids);
                
                // Combine: subfolders first, then endpoints
                const allChildren = [...folderChildren, ...requestsByEndpoint];

                const folderId = ids.next(`folder:${folderPath}`);
                const folderItem = new RequestTreeItem(
                    folderName,
                    null,
                    this.state.collapsibleState(folderId, vscode.TreeItemCollapsibleState.Collapsed),
                    allChildren
                );
                folderItem.id = folderId;
                folderItem.contextValue = 'folder';
                folderItem.iconPath = new vscode.ThemeIcon('folder');
                folderItem.tooltip = `Folder: ${folderName}`;

                items.push(folderItem);
            }

            // If this is root, add root-level requests grouped by endpoint
            if (isRoot) {
                const requestsByEndpoint = this.groupRequestsByEndpointInFolder(node.requests, '', ids);
                items.push(...requestsByEndpoint);
            }

            return items;
        };

        return convertNodeToItems(rootNode, true);
    }

    private groupRequestsByEndpointInFolder(requests: RequestInfo[], folderPath: string, ids: TreeItemIds): RequestTreeItem[] {
        const grouped = new Map<string, RequestInfo[]>();
        const topLevel: RequestInfo[] = [];

        // Group requests by endpoint
        for (const request of requests) {
            if (request.endpoint) {
                if (!grouped.has(request.endpoint)) {
                    grouped.set(request.endpoint, []);
                }
                grouped.get(request.endpoint)!.push(request);
            } else {
                topLevel.push(request);
            }
        }

        const items: RequestTreeItem[] = [];

        // Add endpoint groups
        for (const [endpointName, endpointRequests] of grouped.entries()) {
            const children = endpointRequests.map(req => {
                // Strip endpoint prefix from request name for display
                const displayName = req.name.startsWith(endpointName + '/')
                    ? req.name.substring(endpointName.length + 1)
                    : req.name;
                
                return this.makeRequestItem(displayName, req, ids);
            });

            const endpointId = ids.next(`endpoint:${folderPath}#${endpointName}`);
            const endpointItem = new RequestTreeItem(
                endpointName,
                null,
                this.state.collapsibleState(endpointId, vscode.TreeItemCollapsibleState.Expanded),
                children
            );
            endpointItem.id = endpointId;
            endpointItem.iconPath = new vscode.ThemeIcon('globe');
            const epRef = endpointRequests[0];
            if (epRef.endpoint_file) {
                endpointItem.command = {
                    command: 'rq.openEndpoint',
                    title: 'Go to Endpoint',
                    arguments: [epRef.endpoint_file, epRef.endpoint_line ?? 0, epRef.endpoint_character ?? 0, endpointItem]
                };
            }
            items.push(endpointItem);
        }

        // Add top-level requests (without endpoint) at same level as endpoint nodes
        for (const request of topLevel) {
            items.push(this.makeRequestItem(request.name, request, ids));
        }

        return items;
    }

    private makeRequestItem(label: string, request: RequestInfo, ids: TreeItemIds): RequestTreeItem {
        const item = new RequestTreeItem(label, request, vscode.TreeItemCollapsibleState.None);
        item.id = ids.next(`request:${normalizePath(request.file)}#${request.name}`);
        return item;
    }
}

function findItemById(items: RequestTreeItem[], id: string): RequestTreeItem | undefined {
    for (const item of items) {
        const found = item.id === id ? item : findItemById(item.children ?? [], id);
        if (found) {
            return found;
        }
    }
    return undefined;
}

/**
 * Tree item representing a request or endpoint group
 */
export class RequestTreeItem extends vscode.TreeItem {
    public parent: RequestTreeItem | undefined;

    constructor(
        public label: string,
        public readonly request: RequestInfo | null,
        public readonly collapsibleState: vscode.TreeItemCollapsibleState,
        public readonly children?: RequestTreeItem[]
    ) {
        super(label, collapsibleState);
        children?.forEach(child => child.parent = this);

        if (request) {
            // This is a request item
            this.contextValue = 'request';
            this.tooltip = `${request.name}\nFile: ${request.file}`;
            this.description = undefined;
            
            // Set icon
            this.iconPath = new vscode.ThemeIcon('symbol-interface');
            
            // Make it clickable to open the file
            this.command = {
                command: 'rq.openRequestFile',
                title: 'Open Request File',
                arguments: [request.name, this]
            };
        } else {
            // This is an endpoint group
            this.contextValue = 'endpoint';
            this.tooltip = `Endpoint: ${label}`;
            this.iconPath = new vscode.ThemeIcon('folder');
        }
    }
}
