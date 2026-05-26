import * as vscode from 'vscode';
import { RequestExplorerProvider, RequestTreeItem } from '../src/requestExplorer';
import * as rqClient from '../src/rqClient';

jest.mock('../src/rqClient');

async function getRequestItems(target: RequestExplorerProvider): Promise<RequestTreeItem[]> {
    await target.getChildren();
    await new Promise(r => setImmediate(r));
    const root = target.getChildren() as RequestTreeItem[];
    return root.filter(c => c.contextValue !== 'environment-info');
}

describe('RequestExplorerProvider', () => {
    let target: RequestExplorerProvider;

    beforeEach(() => {
        jest.clearAllMocks();
        target = new RequestExplorerProvider('/root');
    });

    test('refresh() triggers onDidChangeTreeData event', () => {
        const eventSpy = jest.fn();
        target.onDidChangeTreeData(eventSpy);
        target.refresh();
        expect(eventSpy).toHaveBeenCalledTimes(1);
    });

    test('getSelectedEnvironment() returns undefined initially', () => {
        expect(target.getSelectedEnvironment()).toBeUndefined();
    });

    test('setSelectedEnvironment() updates environment and fires change event', async () => {
        await target.getChildren();
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

    test('getChildren() returns env info then loading placeholder before load completes', async () => {
        (rqClient.listRequests as jest.Mock).mockResolvedValue({ requests: [] });

        const first = await target.getChildren() as RequestTreeItem[];
        expect(first.length).toBe(2);
        expect(first[0].contextValue).toBe('environment-info');
        expect(first[1].contextValue).toBe('loading');
    });

    test('getChildren() calls CLI and returns grouped items after load', async () => {
        const mockOutput = [
            { name: 'req1', endpoint: 'GET /api', file: '/root/req1.http' },
            { name: 'req2', endpoint: null, file: '/root/req2.http' }
        ];

        (rqClient.listRequests as jest.Mock).mockResolvedValue({ requests: mockOutput });

        const items = await getRequestItems(target);

        expect(items.length).toBe(2);

        const endpointItem = items.find(c => c.label === 'GET /api');
        expect(endpointItem).toBeDefined();
        expect(endpointItem?.contextValue).toBe('endpoint');

        const reqItem = items.find(c => c.label === 'req2');
        expect(reqItem).toBeDefined();
        expect(reqItem?.contextValue).toBe('request');

        expect(rqClient.listRequests).toHaveBeenCalledWith('/root');
    });

    test('getChildren() handles CLI errors gracefully', async () => {
        (rqClient.listRequests as jest.Mock).mockRejectedValue(new Error('CLI Error'));

        const items = await getRequestItems(target);

        expect(items.length).toBe(1);
        expect(items[0].contextValue).toBe('error');
        expect(items[0].label).toBe('Error loading requests');

        expect(vscode.window.showErrorMessage).toHaveBeenCalledWith(expect.stringContaining('Failed to list requests'));
    });

    test('getChildren() shows parse errors item when CLI reports warnings', async () => {
        const mockOutput = [
            { name: 'req1', endpoint: null, file: '/root/req1.http' }
        ];
        const errors = ['Warning: Failed to parse file1.rq: Syntax error', 'Warning: Failed to parse file2.rq: Syntax error'];
        (rqClient.listRequests as jest.Mock).mockResolvedValue({ requests: mockOutput, errors });

        const items = await getRequestItems(target);

        expect(items.length).toBe(2);

        const errorItem = items.find(c => c.contextValue === 'error')!;
        expect(errorItem.label).toContain('Parse Errors (2)');
        expect(errorItem.tooltip).toContain('file1.rq');
        expect(errorItem.tooltip).toContain('file2.rq');

        const reqItem = items.find(c => c.label === 'req1');
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

        const items = await getRequestItems(target);

        expect(items.length).toBe(1);
        expect(items[0].label).toBe('req1');
        expect(items[0].contextValue).toBe('request');
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

        const items = await getRequestItems(target);

        const endpointItem = items.find(c => c.label === 'api');
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

        const items = await getRequestItems(target);

        const endpointItem = items.find(c => c.label === 'api');
        expect(endpointItem).toBeDefined();
        expect(endpointItem?.command).toBeUndefined();
    });
});
