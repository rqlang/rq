import * as vscode from 'vscode';
import * as rqClient from './rqClient';
import * as path from 'path';
import { normalizePath, applyTreeItemLoading } from './utils';

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
export class RequestExplorerProvider implements vscode.TreeDataProvider<RequestTreeItem> {
    private _onDidChangeTreeData: vscode.EventEmitter<RequestTreeItem | undefined | null | void> = new vscode.EventEmitter<RequestTreeItem | undefined | null | void>();
    readonly onDidChangeTreeData: vscode.Event<RequestTreeItem | undefined | null | void> = this._onDidChangeTreeData.event;
    private _onDidChangeEnvironment: vscode.EventEmitter<string | undefined> = new vscode.EventEmitter<string | undefined>();
    readonly onDidChangeEnvironment: vscode.Event<string | undefined> = this._onDidChangeEnvironment.event;
    private selectedEnvironment: string | undefined;
    private envItem: RequestTreeItem | undefined;
    private readonly originalIcons = new WeakMap<RequestTreeItem, vscode.TreeItem['iconPath']>();
    private cachedRootItems: RequestTreeItem[] | null = null;
    private loadingRootItems = false;

    constructor(private workspaceRoot: string | undefined) {}

    setTreeLoading(loading: boolean): void {
        if (!this.envItem) { return; }
        if (loading) {
            this.envItem.iconPath = new vscode.ThemeIcon('sync~spin');
            this.envItem.description = 'Loading\u2026';
        } else {
            this.envItem.iconPath = new vscode.ThemeIcon(this.selectedEnvironment ? 'server-environment' : 'circle-slash');
            this.envItem.description = this.selectedEnvironment || 'None';
        }
        this._onDidChangeTreeData.fire(this.envItem);
    }

    setItemLoading(item: RequestTreeItem, loading: boolean): void {
        applyTreeItemLoading(item, loading, this.originalIcons, (i) => this._onDidChangeTreeData.fire(i as RequestTreeItem));
    }

    getSelectedEnvironment(): string | undefined {
        return this.selectedEnvironment;
    }

    setSelectedEnvironment(environment: string | undefined): void {
        this.selectedEnvironment = environment;
        this._onDidChangeEnvironment.fire(environment);
        if (this.envItem) {
            this.envItem.iconPath = new vscode.ThemeIcon(environment ? 'server-environment' : 'circle-slash');
            this.envItem.description = environment || 'None';
            this.envItem.tooltip = environment
                ? `Current environment: ${environment}\n\nClick the environment icon in the toolbar to change.`
                : 'No environment selected.\n\nClick the environment icon in the toolbar to select one.';
            this._onDidChangeTreeData.fire(this.envItem);
        }
    }

    refresh(): void {
        this.cachedRootItems = null;
        this._onDidChangeTreeData.fire();
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
            return [this.buildEnvInfoItem(), ...this.getSectionItems()];
        }

        if (element.contextValue === 'section-requests') {
            if (this.cachedRootItems !== null) {
                return this.cachedRootItems;
            }
            this.startLoadingRootItems();
            return [this.makeLoadingItem()];
        }

        if (element.contextValue === 'section-environments') {
            return this.loadEnvironments();
        }

        if (element.contextValue === 'section-auth') {
            return this.loadAuthConfigs();
        }

        return element.children || [];
    }

    private buildEnvInfoItem(): RequestTreeItem {
        const item = new RequestTreeItem('Environment', null, vscode.TreeItemCollapsibleState.None);
        item.contextValue = 'environment-info';
        item.iconPath = new vscode.ThemeIcon(this.selectedEnvironment ? 'server-environment' : 'circle-slash');
        item.description = this.selectedEnvironment || 'None';
        item.tooltip = this.selectedEnvironment
            ? `Current environment: ${this.selectedEnvironment}\n\nClick the environment icon in the toolbar to change.`
            : 'No environment selected.\n\nClick the environment icon in the toolbar to select one.';
        this.envItem = item;
        return item;
    }

    private getSectionItems(): RequestTreeItem[] {
        const requestsSection = new RequestTreeItem('REQUESTS', null, vscode.TreeItemCollapsibleState.Expanded);
        requestsSection.contextValue = 'section-requests';
        requestsSection.iconPath = undefined;
        requestsSection.tooltip = undefined;

        const envsSection = new RequestTreeItem('ENVIRONMENTS', null, vscode.TreeItemCollapsibleState.Collapsed);
        envsSection.contextValue = 'section-environments';
        envsSection.iconPath = undefined;
        envsSection.tooltip = undefined;

        const authSection = new RequestTreeItem('AUTH', null, vscode.TreeItemCollapsibleState.Collapsed);
        authSection.contextValue = 'section-auth';
        authSection.iconPath = undefined;
        authSection.tooltip = undefined;

        return [requestsSection, envsSection, authSection];
    }

    private async loadEnvironments(): Promise<RequestTreeItem[]> {
        try {
            const names = await rqClient.listEnvironments(this.workspaceRoot);
            return names.map(name => {
                const item = new RequestTreeItem(name, null, vscode.TreeItemCollapsibleState.None);
                item.contextValue = 'environment';
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

    private async loadAuthConfigs(): Promise<RequestTreeItem[]> {
        try {
            const entries = await rqClient.listAuthConfigs(this.workspaceRoot);
            return entries.map(e => {
                const item = new RequestTreeItem(e.name, null, vscode.TreeItemCollapsibleState.None);
                item.contextValue = 'auth-config';
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

    private makeLoadingItem(): RequestTreeItem {
        const item = new RequestTreeItem('Loading…', null, vscode.TreeItemCollapsibleState.None);
        item.contextValue = 'loading';
        item.iconPath = new vscode.ThemeIcon('sync~spin');
        return item;
    }

    private async startLoadingRootItems(): Promise<void> {
        if (this.loadingRootItems) { return; }
        this.loadingRootItems = true;
        const items = await this.getRootItems();
        this.loadingRootItems = false;
        this.cachedRootItems = items;
        this._onDidChangeTreeData.fire();
    }

    private async getRootItems(): Promise<RequestTreeItem[]> {
        try {
            const result = await rqClient.listRequests(this.workspaceRoot);
            const requestItems = this.groupRequestsByFolder(result.requests);

            if (result.errors && result.errors.length > 0) {
                const errorItem = new RequestTreeItem(
                    `Parse Errors (${result.errors.length})`,
                    null,
                    vscode.TreeItemCollapsibleState.None
                );
                errorItem.contextValue = 'error';
                errorItem.iconPath = new vscode.ThemeIcon('error');
                errorItem.tooltip = result.errors.join('\n');
                errorItem.description = 'Check Output panel';
                return [errorItem, ...requestItems];
            }

            return requestItems;
        } catch (error) {
            const errorMessage = error instanceof Error ? error.message : 'Unknown error';
            vscode.window.showErrorMessage(`Failed to list requests: ${errorMessage}`);

            const errorItem = new RequestTreeItem(
                'Error loading requests',
                null,
                vscode.TreeItemCollapsibleState.None
            );
            errorItem.contextValue = 'error';
            errorItem.iconPath = new vscode.ThemeIcon('error');
            errorItem.tooltip = errorMessage;
            errorItem.description = 'Check Output panel for details';
            return [errorItem];
        }
    }

    private groupRequestsByFolder(requests: RequestInfo[]): RequestTreeItem[] {
        if (!this.workspaceRoot) {
            return [];
        }

        // Normalize workspace root to handle drive letter casing issues on Windows
        const normalizedRoot = normalizePath(this.workspaceRoot);

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
                const requestsByEndpoint = this.groupRequestsByEndpointInFolder(subNode.requests);
                
                // Combine: subfolders first, then endpoints
                const allChildren = [...folderChildren, ...requestsByEndpoint];

                const folderItem = new RequestTreeItem(
                    folderName,
                    null,
                    vscode.TreeItemCollapsibleState.Collapsed,
                    allChildren
                );
                folderItem.contextValue = 'folder';
                folderItem.iconPath = new vscode.ThemeIcon('folder');
                folderItem.tooltip = `Folder: ${folderName}`;

                items.push(folderItem);
            }

            // If this is root, add root-level requests grouped by endpoint
            if (isRoot) {
                const requestsByEndpoint = this.groupRequestsByEndpointInFolder(node.requests);
                items.push(...requestsByEndpoint);
            }

            return items;
        };

        return convertNodeToItems(rootNode, true);
    }

    private groupRequestsByEndpointInFolder(requests: RequestInfo[]): RequestTreeItem[] {
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
                
                return new RequestTreeItem(
                    displayName,
                    req,
                    vscode.TreeItemCollapsibleState.None
                );
            });

            const endpointItem = new RequestTreeItem(
                endpointName,
                null,
                vscode.TreeItemCollapsibleState.Expanded,
                children
            );
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
            items.push(new RequestTreeItem(
                request.name,
                request,
                vscode.TreeItemCollapsibleState.None
            ));
        }

        return items;
    }
}

/**
 * Tree item representing a request or endpoint group
 */
export class RequestTreeItem extends vscode.TreeItem {
    constructor(
        public readonly label: string,
        public readonly request: RequestInfo | null,
        public readonly collapsibleState: vscode.TreeItemCollapsibleState,
        public readonly children?: RequestTreeItem[]
    ) {
        super(label, collapsibleState);

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
