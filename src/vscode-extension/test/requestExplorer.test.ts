import * as vscode from 'vscode';
import { RequestExplorerProvider, RequestTreeItem } from '../src/requestExplorer';
import * as rqClient from '../src/rqClient';

jest.mock('../src/rqClient');

async function getRequestSectionChildren(target: RequestExplorerProvider): Promise<RequestTreeItem[]> {
    const root = await target.getChildren() as RequestTreeItem[];
    const section = root.find(c => c.contextValue === 'section-requests')!;
    await target.getChildren(section);
    await new Promise(r => setImmediate(r));
    return target.getChildren(section) as RequestTreeItem[];
}

describe('RequestExplorerProvider', () => {
    let target: RequestExplorerProvider;

    beforeEach(() => {
        jest.clearAllMocks();
        target = new RequestExplorerProvider('/root');
    });

    test('refresh() triggers onDidChangeTreeData event', () => {
        // 1. Setup a spy to listen for the event
        const eventSpy = jest.fn();

        // 2. Subscribe to the event
        target.onDidChangeTreeData(eventSpy);

        // 3. Trigger refresh
        target.refresh();

        // 4. Verify the event was fired
        expect(eventSpy).toHaveBeenCalledTimes(1);
    });

    test('getSelectedEnvironment() returns undefined initially', () => {
        expect(target.getSelectedEnvironment()).toBeUndefined();
    });

    test('setSelectedEnvironment() updates environment and triggers refresh', async () => {
        await target.getChildren(); // initializes envItem
        const eventSpy = jest.fn();
        target.onDidChangeTreeData(eventSpy);

        target.setSelectedEnvironment('prod');

        expect(target.getSelectedEnvironment()).toBe('prod');
        expect(eventSpy).toHaveBeenCalledTimes(1);
    });

    test('getTreeItem() returns the element itself', () => {
        const item = new RequestTreeItem('test', null, vscode.TreeItemCollapsibleState.None);
        expect(target.getTreeItem(item)).toBe(item);
    });

    test('getChildren() with element returns element children', async () => {
        const child = new RequestTreeItem('child', null, vscode.TreeItemCollapsibleState.None);
        const parent = new RequestTreeItem('parent', null, vscode.TreeItemCollapsibleState.Expanded, [child]);

        const children = await target.getChildren(parent);
        expect(children).toEqual([child]);
    });

    test('getChildren() with element returns empty array if no children', async () => {
        const parent = new RequestTreeItem('parent', null, vscode.TreeItemCollapsibleState.None);

        const children = await target.getChildren(parent);
        expect(children).toEqual([]);
    });

    test('getChildren() returns empty array and shows message if no workspace root', async () => {
        const noRootProvider = new RequestExplorerProvider(undefined);
        const children = await noRootProvider.getChildren();
        expect(children).toEqual([]);
        expect(vscode.window.showInformationMessage).toHaveBeenCalledWith('No workspace folder open');
    });

    test('getChildren() returns section structure at root level', async () => {
        const root = await target.getChildren() as RequestTreeItem[];
        expect(root.length).toBe(4);
        expect(root[0].contextValue).toBe('environment-info');
        expect(root[1].contextValue).toBe('section-requests');
        expect(root[2].contextValue).toBe('section-environments');
        expect(root[3].contextValue).toBe('section-auth');
    });

    test('getChildren() calls CLI and returns grouped items', async () => {
        const mockOutput = [
            { name: 'req1', endpoint: 'GET /api', file: '/root/req1.http' },
            { name: 'req2', endpoint: null, file: '/root/req2.http' }
        ];

        (rqClient.listRequests as jest.Mock).mockResolvedValue({ requests: mockOutput });

        const children = await getRequestSectionChildren(target);

        expect(children.length).toBe(2);

        const endpointItem = children.find(c => c.label === 'GET /api');
        expect(endpointItem).toBeDefined();
        expect(endpointItem?.contextValue).toBe('endpoint');

        const reqItem = children.find(c => c.label === 'req2');
        expect(reqItem).toBeDefined();
        expect(reqItem?.contextValue).toBe('request');

        expect(rqClient.listRequests).toHaveBeenCalledWith('/root');
    });

    test('getChildren() handles CLI errors gracefully', async () => {
        (rqClient.listRequests as jest.Mock).mockRejectedValue(new Error('CLI Error'));

        const children = await getRequestSectionChildren(target);

        expect(children.length).toBe(1);
        expect(children[0].contextValue).toBe('error');
        expect(children[0].label).toBe('Error loading requests');

        expect(vscode.window.showErrorMessage).toHaveBeenCalledWith(expect.stringContaining('Failed to list requests'));
    });

    test('getChildren() shows parse errors item when CLI reports warnings', async () => {
        const mockOutput = [
            { name: 'req1', endpoint: null, file: '/root/req1.http' }
        ];
        const errors = ['Warning: Failed to parse file1.rq: Syntax error', 'Warning: Failed to parse file2.rq: Syntax error'];
        (rqClient.listRequests as jest.Mock).mockResolvedValue({ requests: mockOutput, errors });

        const children = await getRequestSectionChildren(target);

        expect(children.length).toBe(2);

        const errorItem = children.find(c => c.contextValue === 'error')!;
        expect(errorItem.label).toContain('Parse Errors (2)');
        expect(errorItem.tooltip).toContain('file1.rq');
        expect(errorItem.tooltip).toContain('file2.rq');

        const reqItem = children.find(c => c.label === 'req1');
        expect(reqItem).toBeDefined();
    });

    test('setItemLoading(true) saves original icon and sets spinner', () => {
        const item = new RequestTreeItem('req', { name: 'req', endpoint: null, file: '/root/req.rq' }, vscode.TreeItemCollapsibleState.None);
        const originalIcon = item.iconPath;

        target.setItemLoading(item, true);

        expect(item.iconPath).toEqual(new vscode.ThemeIcon('sync~spin'));
        expect(item.iconPath).not.toBe(originalIcon);
    });

    test('setItemLoading(false) restores original icon', () => {
        const item = new RequestTreeItem('req', { name: 'req', endpoint: null, file: '/root/req.rq' }, vscode.TreeItemCollapsibleState.None);
        const originalIcon = item.iconPath;

        target.setItemLoading(item, true);
        target.setItemLoading(item, false);

        expect(item.iconPath).toEqual(originalIcon);
    });

    test('setItemLoading fires onDidChangeTreeData for the item', () => {
        const item = new RequestTreeItem('req', { name: 'req', endpoint: null, file: '/root/req.rq' }, vscode.TreeItemCollapsibleState.None);
        const eventSpy = jest.fn();
        target.onDidChangeTreeData(eventSpy);

        target.setItemLoading(item, true);
        expect(eventSpy).toHaveBeenCalledWith(item);

        target.setItemLoading(item, false);
        expect(eventSpy).toHaveBeenCalledWith(item);
        expect(eventSpy).toHaveBeenCalledTimes(2);
    });

    test('getChildren() handles files outside workspace root gracefully', async () => {
        const mockOutput = [
            { name: 'req1', endpoint: null, file: '/outside/req1.http' }
        ];

        (rqClient.listRequests as jest.Mock).mockResolvedValue({ requests: mockOutput });

        const children = await getRequestSectionChildren(target);

        expect(children.length).toBe(1);
        expect(children[0].label).toBe('req1');
        expect(children[0].contextValue).toBe('request');
    });

    test('endpoint item has rq.openEndpoint command when endpoint_file is present', async () => {
        const mockOutput = [
            {
                name: 'api/get',
                endpoint: 'api',
                file: '/root/api.rq',
                endpoint_file: '/root/api.rq',
                endpoint_line: 5,
                endpoint_character: 0
            }
        ];

        (rqClient.listRequests as jest.Mock).mockResolvedValue({ requests: mockOutput });

        const children = await getRequestSectionChildren(target);

        const endpointItem = children.find(c => c.label === 'api');
        expect(endpointItem).toBeDefined();
        expect(endpointItem?.contextValue).toBe('endpoint');
        expect(endpointItem?.command).toBeDefined();
        expect(endpointItem?.command?.command).toBe('rq.openEndpoint');
        expect(endpointItem?.command?.arguments?.[0]).toBe('/root/api.rq');
        expect(endpointItem?.command?.arguments?.[1]).toBe(5);
        expect(endpointItem?.command?.arguments?.[2]).toBe(0);
        expect(endpointItem?.command?.arguments?.[3]).toBe(endpointItem);
    });

    test('endpoint item has no command when endpoint_file is absent', async () => {
        const mockOutput = [
            {
                name: 'api/get',
                endpoint: 'api',
                file: '/root/api.rq'
            }
        ];

        (rqClient.listRequests as jest.Mock).mockResolvedValue({ requests: mockOutput });

        const children = await getRequestSectionChildren(target);

        const endpointItem = children.find(c => c.label === 'api');
        expect(endpointItem).toBeDefined();
        expect(endpointItem?.command).toBeUndefined();
    });

    test('getChildren(section-environments) returns environment items', async () => {
        (rqClient.listEnvironments as jest.Mock).mockResolvedValue(['local', 'prod']);

        const root = await target.getChildren() as RequestTreeItem[];
        const section = root.find(c => c.contextValue === 'section-environments')!;
        const children = await target.getChildren(section) as RequestTreeItem[];

        expect(children.length).toBe(2);
        expect(children[0].label).toBe('local');
        expect(children[0].contextValue).toBe('environment');
        expect(children[0].command?.command).toBe('rq.openConfigurationFile');
        expect(children[0].command?.arguments?.[0]).toBe('env');
        expect(children[0].command?.arguments?.[1]).toBe('local');
        expect(children[0].command?.arguments?.[2]).toBe(children[0]);
        expect(children[1].label).toBe('prod');
    });

    test('getChildren(section-auth) returns auth config items', async () => {
        (rqClient.listAuthConfigs as jest.Mock).mockResolvedValue([
            { name: 'my-token', auth_type: 'Bearer' },
            { name: 'api-key', auth_type: 'ApiKey' }
        ]);

        const root = await target.getChildren() as RequestTreeItem[];
        const section = root.find(c => c.contextValue === 'section-auth')!;
        const children = await target.getChildren(section) as RequestTreeItem[];

        expect(children.length).toBe(2);
        expect(children[0].label).toBe('my-token');
        expect(children[0].contextValue).toBe('auth-config');
        expect(children[0].description).toBe('Bearer');
        expect(children[0].command?.command).toBe('rq.openConfigurationFile');
        expect(children[0].command?.arguments?.[0]).toBe('auth');
        expect(children[0].command?.arguments?.[1]).toBe('my-token');
        expect(children[0].command?.arguments?.[2]).toBe(children[0]);
        expect(children[1].label).toBe('api-key');
    });

    test('getChildren(section-environments) handles CLI error gracefully', async () => {
        (rqClient.listEnvironments as jest.Mock).mockRejectedValue(new Error('CLI Error'));

        const root = await target.getChildren() as RequestTreeItem[];
        const section = root.find(c => c.contextValue === 'section-environments')!;
        const children = await target.getChildren(section) as RequestTreeItem[];

        expect(children).toEqual([]);
        expect(vscode.window.showErrorMessage).toHaveBeenCalledWith(expect.stringContaining('Failed to load environments'));
    });

    test('getChildren(section-auth) handles CLI error gracefully', async () => {
        (rqClient.listAuthConfigs as jest.Mock).mockRejectedValue(new Error('CLI Error'));

        const root = await target.getChildren() as RequestTreeItem[];
        const section = root.find(c => c.contextValue === 'section-auth')!;
        const children = await target.getChildren(section) as RequestTreeItem[];

        expect(children).toEqual([]);
        expect(vscode.window.showErrorMessage).toHaveBeenCalledWith(expect.stringContaining('Failed to load auth configs'));
    });
});
