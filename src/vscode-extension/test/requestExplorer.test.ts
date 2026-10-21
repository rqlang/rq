import * as vscode from 'vscode';
import { RequestExplorerProvider, RequestTreeItem } from '../src/requestExplorer';
import * as rqClient from '../src/rqClient';
import { FakeMemento, createExplorerState } from './memento-mock';

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
        target = new RequestExplorerProvider('/root', createExplorerState());
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
        const noRootProvider = new RequestExplorerProvider(undefined, createExplorerState());
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

    test('setItemRunning(true) switches the context value to request-running', () => {
        const item = new RequestTreeItem('req', { name: 'req', endpoint: null, file: '/root/req.rq' }, vscode.TreeItemCollapsibleState.None);

        target.setItemRunning(item, true);

        expect(item.contextValue).toBe('request-running');
        expect(item.iconPath).toEqual(new vscode.ThemeIcon('sync~spin'));
    });

    test('setItemRunning(false) restores the request context value and icon', () => {
        const item = new RequestTreeItem('req', { name: 'req', endpoint: null, file: '/root/req.rq' }, vscode.TreeItemCollapsibleState.None);
        const originalIcon = item.iconPath;

        target.setItemRunning(item, true);
        target.setItemRunning(item, false);

        expect(item.contextValue).toBe('request');
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

    test('restores the selected environment from the stored state', () => {
        const memento = new FakeMemento();
        new RequestExplorerProvider('/root', createExplorerState(memento)).setSelectedEnvironment('prod');

        const restored = new RequestExplorerProvider('/root', createExplorerState(memento));

        expect(restored.getSelectedEnvironment()).toBe('prod');
    });

    test('assigns stable ids to folders, endpoints and requests', async () => {
        (rqClient.listRequests as jest.Mock).mockResolvedValue({ requests: [
            { name: 'api/get', endpoint: 'api', file: '/root/users/api.rq' }
        ] });

        const [folder] = await getRequestItems(target);
        const endpoint = folder.children![0];

        expect(folder.id).toBe('folder:users');
        expect(endpoint.id).toBe('endpoint:users#api');
        expect(endpoint.children![0].id).toBe('request:/root/users/api.rq#api/get');
    });

    test('makes duplicated request ids unique', async () => {
        (rqClient.listRequests as jest.Mock).mockResolvedValue({ requests: [
            { name: 'get', endpoint: null, file: '/root/a.rq' },
            { name: 'get', endpoint: null, file: '/root/a.rq' }
        ] });

        const items = await getRequestItems(target);

        expect(items.map(i => i.id)).toEqual(['request:/root/a.rq#get', 'request:/root/a.rq#get~2']);
    });

    test('builds items with the stored expansion state', async () => {
        const state = createExplorerState();
        state.setExpanded('folder:users', true);
        state.setExpanded('endpoint:users#api', false);
        target = new RequestExplorerProvider('/root', state);
        (rqClient.listRequests as jest.Mock).mockResolvedValue({ requests: [
            { name: 'api/get', endpoint: 'api', file: '/root/users/api.rq' }
        ] });

        const [folder] = await getRequestItems(target);

        expect(folder.collapsibleState).toBe(vscode.TreeItemCollapsibleState.Expanded);
        expect(folder.children![0].collapsibleState).toBe(vscode.TreeItemCollapsibleState.Collapsed);
    });

    test('refresh() after a load keeps the current items until the reload completes', async () => {
        (rqClient.listRequests as jest.Mock).mockResolvedValue({ requests: [{ name: 'req1', endpoint: null, file: '/root/req1.rq' }] });
        await getRequestItems(target);

        target.refresh();
        const during = target.getChildren() as RequestTreeItem[];

        expect(during.map(i => i.label)).toEqual(['Environment', 'req1']);
        expect(rqClient.listRequests).toHaveBeenCalledTimes(2);
    });

    test('refresh() during a load reloads once more when it finishes', async () => {
        (rqClient.listRequests as jest.Mock).mockResolvedValue({ requests: [] });
        target.getChildren();

        target.refresh();
        await new Promise(r => setImmediate(r));

        expect(rqClient.listRequests).toHaveBeenCalledTimes(2);
    });

    test('fires onDidLoad once the requests are loaded', async () => {
        (rqClient.listRequests as jest.Mock).mockResolvedValue({ requests: [] });
        const loadSpy = jest.fn();
        target.onDidLoad(loadSpy);

        await getRequestItems(target);

        expect(loadSpy).toHaveBeenCalledTimes(1);
    });

    test('findItem() finds a nested request and getParent() walks up to the root', async () => {
        (rqClient.listRequests as jest.Mock).mockResolvedValue({ requests: [
            { name: 'api/get', endpoint: 'api', file: '/root/users/api.rq' }
        ] });
        const [folder] = await getRequestItems(target);

        const request = target.findItem('request:/root/users/api.rq#api/get')!;
        const endpoint = target.getParent(request)!;

        expect(endpoint.id).toBe('endpoint:users#api');
        expect(target.getParent(endpoint)).toBe(folder);
        expect(target.getParent(folder)).toBeUndefined();
    });

    test('isLoaded() becomes true once the requests are loaded', async () => {
        (rqClient.listRequests as jest.Mock).mockResolvedValue({ requests: [] });
        expect(target.isLoaded()).toBe(false);

        await getRequestItems(target);

        expect(target.isLoaded()).toBe(true);
    });

    test('marks a restored environment that no longer exists', async () => {
        const state = createExplorerState();
        state.setEnvironment('dev');
        target = new RequestExplorerProvider('/root', state);
        (rqClient.listRequests as jest.Mock).mockResolvedValue({ requests: [] });
        (rqClient.listEnvironmentsWithErrors as jest.Mock).mockResolvedValue({ environments: [], errors: [] });

        await getRequestItems(target);
        const envItem = (target.getChildren() as RequestTreeItem[])[0];

        expect(target.getSelectedEnvironment()).toBe('dev');
        expect(envItem.description).toBe('dev (not found)');
        expect(envItem.iconPath).toEqual(new vscode.ThemeIcon('warning'));
    });

    test('does not mark a restored environment that still exists', async () => {
        const state = createExplorerState();
        state.setEnvironment('dev');
        target = new RequestExplorerProvider('/root', state);
        (rqClient.listRequests as jest.Mock).mockResolvedValue({ requests: [] });
        (rqClient.listEnvironmentsWithErrors as jest.Mock).mockResolvedValue({ environments: ['dev'], errors: [] });

        await getRequestItems(target);
        const envItem = (target.getChildren() as RequestTreeItem[])[0];

        expect(envItem.description).toBe('dev');
        expect(envItem.iconPath).toEqual(new vscode.ThemeIcon('server-environment'));
    });

    test('does not mark the environment when the environments cannot be listed', async () => {
        const state = createExplorerState();
        state.setEnvironment('dev');
        target = new RequestExplorerProvider('/root', state);
        (rqClient.listRequests as jest.Mock).mockResolvedValue({ requests: [] });
        (rqClient.listEnvironmentsWithErrors as jest.Mock).mockRejectedValue(new Error('parse error'));

        await getRequestItems(target);
        const envItem = (target.getChildren() as RequestTreeItem[])[0];

        expect(envItem.description).toBe('dev');
    });

    test('clears the missing mark when another environment is selected', async () => {
        const state = createExplorerState();
        state.setEnvironment('dev');
        target = new RequestExplorerProvider('/root', state);
        (rqClient.listRequests as jest.Mock).mockResolvedValue({ requests: [] });
        (rqClient.listEnvironmentsWithErrors as jest.Mock).mockResolvedValue({ environments: ['prod'], errors: [] });
        await getRequestItems(target);

        target.setSelectedEnvironment('prod');
        const envItem = (target.getChildren() as RequestTreeItem[])[0];

        expect(envItem.description).toBe('prod');
    });

    test('isLoaded() stays false when some files fail to parse', async () => {
        (rqClient.listRequests as jest.Mock).mockResolvedValue({ requests: [], errors: ['Failed to parse a.rq'] });

        await getRequestItems(target);

        expect(target.isLoaded()).toBe(false);
    });

    test('isLoaded() stays false when the requests cannot be listed', async () => {
        (rqClient.listRequests as jest.Mock).mockRejectedValue(new Error('CLI Error'));

        await getRequestItems(target);

        expect(target.isLoaded()).toBe(false);
    });

    test('refreshInBackground() does nothing before the first load', () => {
        const eventSpy = jest.fn();
        target.onDidChangeTreeData(eventSpy);

        target.refreshInBackground();

        expect(eventSpy).not.toHaveBeenCalled();
        expect(rqClient.listRequests).not.toHaveBeenCalled();
    });

    test('refreshInBackground() does not show an error when the reload fails', async () => {
        (rqClient.listRequests as jest.Mock).mockResolvedValue({ requests: [] });
        await getRequestItems(target);
        (rqClient.listRequests as jest.Mock).mockRejectedValue(new Error('CLI Error'));

        target.refreshInBackground();
        await new Promise(r => setImmediate(r));

        expect(vscode.window.showErrorMessage).not.toHaveBeenCalled();
    });

    test('refresh() shows an error when the reload fails', async () => {
        (rqClient.listRequests as jest.Mock).mockResolvedValue({ requests: [] });
        await getRequestItems(target);
        (rqClient.listRequests as jest.Mock).mockRejectedValue(new Error('CLI Error'));

        target.refresh();
        await new Promise(r => setImmediate(r));

        expect(vscode.window.showErrorMessage).toHaveBeenCalledWith(expect.stringContaining('Failed to list requests'));
    });

    test('refresh() shows the loading state in the environment row while reloading', async () => {
        (rqClient.listRequests as jest.Mock).mockResolvedValue({ requests: [] });
        await getRequestItems(target);
        const envItem = (target.getChildren() as RequestTreeItem[])[0];

        target.refresh();

        expect(envItem.description).toBe('Loading\u2026');
    });

    test('warnIfEnvironmentMissing() warns about and marks a restored environment that no longer exists', async () => {
        const state = createExplorerState();
        state.setEnvironment('dev');
        target = new RequestExplorerProvider('/root', state);
        (rqClient.listEnvironmentsWithErrors as jest.Mock).mockResolvedValue({ environments: ['prod'], errors: [] });
        (rqClient.listRequests as jest.Mock).mockResolvedValue({ requests: [] });
        const envItem = (target.getChildren() as RequestTreeItem[])[0];

        await target.warnIfEnvironmentMissing();

        expect(envItem.description).toBe('dev (not found)');
        expect(vscode.window.showWarningMessage).toHaveBeenCalledWith(expect.stringContaining("Environment 'dev' no longer exists"));
    });

    test('warnIfEnvironmentMissing() does not warn when the environment exists', async () => {
        const state = createExplorerState();
        state.setEnvironment('dev');
        target = new RequestExplorerProvider('/root', state);
        (rqClient.listEnvironmentsWithErrors as jest.Mock).mockResolvedValue({ environments: ['dev'], errors: [] });

        await target.warnIfEnvironmentMissing();

        expect(vscode.window.showWarningMessage).not.toHaveBeenCalled();
    });

    test('selecting the missing environment again clears its missing mark', async () => {
        const state = createExplorerState();
        state.setEnvironment('dev');
        target = new RequestExplorerProvider('/root', state);
        (rqClient.listRequests as jest.Mock).mockResolvedValue({ requests: [] });
        (rqClient.listEnvironmentsWithErrors as jest.Mock).mockResolvedValue({ environments: [], errors: [] });
        await getRequestItems(target);

        target.setSelectedEnvironment('dev');
        const envItem = (target.getChildren() as RequestTreeItem[])[0];

        expect(envItem.description).toBe('dev');
    });

    test('does not mark the environment as missing while some files fail to parse', async () => {
        const state = createExplorerState();
        state.setEnvironment('dev');
        target = new RequestExplorerProvider('/root', state);
        (rqClient.listRequests as jest.Mock).mockResolvedValue({ requests: [] });
        (rqClient.listEnvironmentsWithErrors as jest.Mock).mockResolvedValue({ environments: [], errors: ['Failed to parse env.rq'] });

        await getRequestItems(target);
        const envItem = (target.getChildren() as RequestTreeItem[])[0];

        expect(envItem.description).toBe('dev');
    });

    test('warnIfEnvironmentMissing() does not warn while some files fail to parse', async () => {
        const state = createExplorerState();
        state.setEnvironment('dev');
        target = new RequestExplorerProvider('/root', state);
        (rqClient.listEnvironmentsWithErrors as jest.Mock).mockResolvedValue({ environments: [], errors: ['Failed to parse env.rq'] });

        await target.warnIfEnvironmentMissing();

        expect(vscode.window.showWarningMessage).not.toHaveBeenCalled();
    });

    test('refreshInBackground() during the first load reloads once more when it finishes', async () => {
        (rqClient.listRequests as jest.Mock).mockResolvedValue({ requests: [] });
        target.getChildren();

        target.refreshInBackground();
        await new Promise(r => setImmediate(r));

        expect(rqClient.listRequests).toHaveBeenCalledTimes(2);
    });

    test('findItem() finds the environment row', () => {
        const [envItem] = target.getChildren() as RequestTreeItem[];

        expect(target.findItem('environment')).toBe(envItem);
    });
});
