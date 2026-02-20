import * as vscode from 'vscode';
import * as cliService from './cliService';
import * as path from 'path';
import { normalizePath } from './utils';

const CLI_NOT_INSTALLED_MSG = 'rq CLI is not installed. Use the "Install Now" prompt or install it manually.';

export interface RequestInfo {
    name: string;
    endpoint: string | null;
    file: string;
}



/**
 * Tree data provider for RQ requests explorer
 */
export class RequestExplorerProvider implements vscode.TreeDataProvider<RequestTreeItem> {
    private _onDidChangeTreeData: vscode.EventEmitter<RequestTreeItem | undefined | null | void> = new vscode.EventEmitter<RequestTreeItem | undefined | null | void>();
    readonly onDidChangeTreeData: vscode.Event<RequestTreeItem | undefined | null | void> = this._onDidChangeTreeData.event;
    private selectedEnvironment: string | undefined;

    constructor(private workspaceRoot: string | undefined) {}

    getSelectedEnvironment(): string | undefined {
        return this.selectedEnvironment;
    }

    setSelectedEnvironment(environment: string | undefined): void {
        this.selectedEnvironment = environment;
        this.refresh();
    }

    refresh(): void {
        this._onDidChangeTreeData.fire();
    }

    getTreeItem(element: RequestTreeItem): vscode.TreeItem {
        return element;
    }

    async getChildren(element?: RequestTreeItem): Promise<RequestTreeItem[]> {
        if (!this.workspaceRoot) {
            vscode.window.showInformationMessage('No workspace folder open');
            return [];
        }

        if (element) {
            // Return requests for this endpoint
            return element.children || [];
        } else {
            // Root level - get all requests and group by endpoint
            return this.getRootItems();
        }
    }

    private async getRootItems(): Promise<RequestTreeItem[]> {
        const items: RequestTreeItem[] = [];
        
        // Add environment info item at the top
        const envLabel = this.selectedEnvironment 
            ? `Environment`
            : 'Environment';
        
        const envItem = new RequestTreeItem(
            envLabel,
            null,
            vscode.TreeItemCollapsibleState.None
        );
        envItem.contextValue = 'environment-info';
        envItem.iconPath = new vscode.ThemeIcon(
            this.selectedEnvironment ? 'server-environment' : 'circle-slash'
        );
        envItem.description = this.selectedEnvironment || 'None';
        envItem.tooltip = this.selectedEnvironment
            ? `Current environment: ${this.selectedEnvironment}\n\nClick the environment icon in the toolbar to change.`
            : 'No environment selected.\n\nClick the environment icon in the toolbar to select one.';
        
        items.push(envItem);

        // If the CLI is currently being installed, show a placeholder
        if (cliService.isCliInstalling()) {
            const installingItem = new RequestTreeItem(
                'Installing rq CLI…',
                null,
                vscode.TreeItemCollapsibleState.None
            );
            installingItem.contextValue = 'info';
            installingItem.iconPath = new vscode.ThemeIcon('sync~spin');
            installingItem.description = 'Please wait';
            installingItem.tooltip = 'The rq CLI is being installed. The explorer will refresh automatically when it is ready.';
            items.push(installingItem);
            return items;
        }

        try {
            // Get and group requests
            const result = await cliService.listRequests(this.workspaceRoot);
            const requestItems = this.groupRequestsByFolder(result.requests);
            
            const finalItems = [...items, ...requestItems];

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
                
                // Insert after environment item
                finalItems.splice(1, 0, errorItem);
            }
            
            return finalItems;
        } catch (error) {
            // Show a friendly message when the CLI is simply not installed
            if (cliService.isCliNotFound(error)) {
                const notInstalledItem = new RequestTreeItem(
                    'rq CLI not installed',
                    null,
                    vscode.TreeItemCollapsibleState.None
                );
                notInstalledItem.iconPath = new vscode.ThemeIcon('warning');
                notInstalledItem.description = 'Click refresh button to install';
                notInstalledItem.tooltip = CLI_NOT_INSTALLED_MSG;
                items.push(notInstalledItem);
                return items;
            }

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
            
            items.push(errorItem);
            return items;
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
                arguments: [request.file, request.name]
            };
        } else {
            // This is an endpoint group
            this.contextValue = 'endpoint';
            this.tooltip = `Endpoint: ${label}`;
            this.iconPath = new vscode.ThemeIcon('folder');
        }
    }
}
