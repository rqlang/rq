import * as vscode from 'vscode';
import { ConfigurationExplorerProvider, ConfigurationTreeItem } from '../src/configurationExplorer';
import * as rqClient from '../src/rqClient';
import { createExplorerState } from './memento-mock';

jest.mock('../src/rqClient');

async function getSectionChildren(
    target: ConfigurationExplorerProvider,
    kind: 'section-environments' | 'section-auth',
): Promise<ConfigurationTreeItem[]> {
    const root = await target.getChildren();
    const section = root.find(c => c.kind === kind)!;
    await target.getChildren(section);
    await new Promise(r => setImmediate(r));
    return target.getChildren(section);
}

describe('ConfigurationExplorerProvider', () => {
    let target: ConfigurationExplorerProvider;

    beforeEach(() => {
        jest.clearAllMocks();
        target = new ConfigurationExplorerProvider('/root', createExplorerState());
    });

    test('refresh() triggers onDidChangeTreeData event', () => {
        const eventSpy = jest.fn();
        target.onDidChangeTreeData(eventSpy);
        target.refresh();
        expect(eventSpy).toHaveBeenCalledTimes(1);
    });

    test('getTreeItem() returns the element itself', () => {
        const item = new ConfigurationTreeItem('Environments', 'section-environments', vscode.TreeItemCollapsibleState.Collapsed);
        expect(target.getTreeItem(item)).toBe(item);
    });

    test('getChildren() returns empty array if no workspace root', async () => {
        const noRootProvider = new ConfigurationExplorerProvider(undefined, createExplorerState());
        const children = await noRootProvider.getChildren();
        expect(children).toEqual([]);
    });

    test('getChildren() returns section items at root level', async () => {
        const children = await target.getChildren() as ConfigurationTreeItem[];
        expect(children.length).toBe(2);
        expect(children[0].label).toBe('Environments');
        expect(children[0].kind).toBe('section-environments');
        expect(children[1].label).toBe('Auth');
        expect(children[1].kind).toBe('section-auth');
    });

    test('getChildren(section-environments) returns loading item synchronously then real items after load', async () => {
        (rqClient.listEnvironments as jest.Mock).mockResolvedValue(['local', 'prod']);

        const root = await target.getChildren() as ConfigurationTreeItem[];
        const section = root.find(c => c.kind === 'section-environments')!;

        const first = await target.getChildren(section);
        expect(first.length).toBe(1);
        expect(first[0].kind).toBe('loading');

        await new Promise(r => setImmediate(r));

        const second = await target.getChildren(section);
        expect(second.length).toBe(2);
        expect(second[0].label).toBe('local');
        expect(second[0].kind).toBe('environment');
        expect(second[0].command?.command).toBe('rq.openConfigurationFile');
        expect(second[0].command?.arguments?.[0]).toBe('env');
        expect(second[0].command?.arguments?.[1]).toBe('local');
        expect(second[0].command?.arguments?.[2]).toBe(second[0]);
        expect(second[1].label).toBe('prod');
    });

    test('getChildren(section-auth) returns auth config items', async () => {
        (rqClient.listAuthConfigs as jest.Mock).mockResolvedValue([
            { name: 'my-token', auth_type: 'Bearer' },
            { name: 'api-key', auth_type: 'ApiKey' }
        ]);

        const children = await getSectionChildren(target, 'section-auth');

        expect(children.length).toBe(2);
        expect(children[0].label).toBe('my-token');
        expect(children[0].kind).toBe('auth-config');
        expect(children[0].description).toBe('Bearer');
        expect(children[0].command?.command).toBe('rq.openConfigurationFile');
        expect(children[0].command?.arguments?.[0]).toBe('auth');
        expect(children[0].command?.arguments?.[1]).toBe('my-token');
        expect(children[0].command?.arguments?.[2]).toBe(children[0]);
        expect(children[1].label).toBe('api-key');
    });

    test('getChildren(section-environments) handles CLI error gracefully', async () => {
        (rqClient.listEnvironments as jest.Mock).mockRejectedValue(new Error('CLI Error'));

        const children = await getSectionChildren(target, 'section-environments');

        expect(children).toEqual([]);
        expect(vscode.window.showErrorMessage).toHaveBeenCalledWith(expect.stringContaining('Failed to load environments'));
    });

    test('getChildren(section-auth) handles CLI error gracefully', async () => {
        (rqClient.listAuthConfigs as jest.Mock).mockRejectedValue(new Error('CLI Error'));

        const children = await getSectionChildren(target, 'section-auth');

        expect(children).toEqual([]);
        expect(vscode.window.showErrorMessage).toHaveBeenCalledWith(expect.stringContaining('Failed to load auth configs'));
    });

    test('cached environments are returned on subsequent expand without re-fetching', async () => {
        (rqClient.listEnvironments as jest.Mock).mockResolvedValue(['local']);

        await getSectionChildren(target, 'section-environments');
        const second = await getSectionChildren(target, 'section-environments');

        expect(second.length).toBe(1);
        expect(rqClient.listEnvironments).toHaveBeenCalledTimes(1);
    });

    test('refresh() reloads a loaded section without showing the loading item', async () => {
        (rqClient.listEnvironments as jest.Mock).mockResolvedValue(['local']);
        await getSectionChildren(target, 'section-environments');
        const section = (await target.getChildren()).find(c => c.kind === 'section-environments')!;

        target.refresh();
        const during = await target.getChildren(section);

        expect(during.map(i => i.label)).toEqual(['local']);
        expect(rqClient.listEnvironments).toHaveBeenCalledTimes(2);
    });

    test('refresh() does not load a section that was never expanded', async () => {
        (rqClient.listEnvironments as jest.Mock).mockResolvedValue(['local']);
        await getSectionChildren(target, 'section-environments');

        target.refresh();

        expect(rqClient.listAuthConfigs).not.toHaveBeenCalled();
    });

    test('builds sections with the stored expansion state', async () => {
        const state = createExplorerState();
        state.setExpanded('section-auth', true);
        target = new ConfigurationExplorerProvider('/root', state);

        const [environments, auth] = await target.getChildren();

        expect(environments.collapsibleState).toBe(vscode.TreeItemCollapsibleState.Collapsed);
        expect(auth.collapsibleState).toBe(vscode.TreeItemCollapsibleState.Expanded);
    });

    test('assigns stable ids to sections and their items', async () => {
        (rqClient.listAuthConfigs as jest.Mock).mockResolvedValue([{ name: 'my-token', auth_type: 'Bearer' }]);

        const [item] = await getSectionChildren(target, 'section-auth');

        expect(item.id).toBe('auth-config:my-token');
        expect(target.getParent(item)?.id).toBe('section-auth');
    });

    test('findItem() finds a loaded environment', async () => {
        (rqClient.listEnvironments as jest.Mock).mockResolvedValue(['local']);
        const [local] = await getSectionChildren(target, 'section-environments');

        expect(target.findItem('environment:local')).toBe(local);
    });

    test('setItemLoading(true) saves original icon and sets spinner', () => {
        const item = new ConfigurationTreeItem('local', 'environment', vscode.TreeItemCollapsibleState.None);
        const originalIcon = item.iconPath;

        target.setItemLoading(item, true);

        expect(item.iconPath).toEqual(new vscode.ThemeIcon('sync~spin'));
        expect(item.iconPath).not.toBe(originalIcon);
    });

    test('setItemLoading(false) restores original icon', () => {
        const item = new ConfigurationTreeItem('local', 'environment', vscode.TreeItemCollapsibleState.None);
        item.iconPath = new vscode.ThemeIcon('server-environment');
        const originalIcon = item.iconPath;

        target.setItemLoading(item, true);
        target.setItemLoading(item, false);

        expect(item.iconPath).toEqual(originalIcon);
    });

    test('setItemLoading fires onDidChangeTreeData for the item', () => {
        const item = new ConfigurationTreeItem('local', 'environment', vscode.TreeItemCollapsibleState.None);
        const eventSpy = jest.fn();
        target.onDidChangeTreeData(eventSpy);

        target.setItemLoading(item, true);
        expect(eventSpy).toHaveBeenCalledWith(item);

        target.setItemLoading(item, false);
        expect(eventSpy).toHaveBeenCalledWith(item);
        expect(eventSpy).toHaveBeenCalledTimes(2);
    });

    test('isLoaded() is true only for items of a loaded section', async () => {
        (rqClient.listEnvironments as jest.Mock).mockResolvedValue(['local']);
        await getSectionChildren(target, 'section-environments');

        expect(target.isLoaded('environment:gone')).toBe(true);
        expect(target.isLoaded('auth-config:my-token')).toBe(false);
    });
});
