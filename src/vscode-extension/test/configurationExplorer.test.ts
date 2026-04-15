import * as vscode from 'vscode';
import { ConfigurationExplorerProvider, ConfigurationTreeItem } from '../src/configurationExplorer';
import * as cp from 'child_process';
import { ShellMock } from './shell-mock';

jest.mock('child_process');

describe('ConfigurationExplorerProvider', () => {
    let target: ConfigurationExplorerProvider;
    let shellMock: ShellMock;

    beforeEach(() => {
        shellMock = new ShellMock();
        target = new ConfigurationExplorerProvider('/root');
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
        const noRootProvider = new ConfigurationExplorerProvider(undefined);
        const children = await noRootProvider.getChildren();
        expect(children).toEqual([]);
    });

    test('getChildren() returns section items at root level', async () => {
        const children = await target.getChildren();
        expect(children.length).toBe(2);
        expect(children[0].label).toBe('Environments');
        expect(children[0].contextValue).toBe('section-environments');
        expect(children[1].label).toBe('Auth');
        expect(children[1].contextValue).toBe('section-auth');
    });

    test('getChildren(section-environments) returns environment items', async () => {
        shellMock.setCommandOutput('env list', [{ name: 'local' }, { name: 'prod' }]);

        const sectionItem = new ConfigurationTreeItem('Environments', 'section-environments', vscode.TreeItemCollapsibleState.Collapsed);
        const children = await target.getChildren(sectionItem);

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
        shellMock.setCommandOutput('auth list', [
            { name: 'my-token', auth_type: 'Bearer' },
            { name: 'api-key', auth_type: 'ApiKey' }
        ]);

        const sectionItem = new ConfigurationTreeItem('Auth', 'section-auth', vscode.TreeItemCollapsibleState.Collapsed);
        const children = await target.getChildren(sectionItem);

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
        shellMock.setCommandError('env list', 'CLI Error');

        const sectionItem = new ConfigurationTreeItem('Environments', 'section-environments', vscode.TreeItemCollapsibleState.Collapsed);
        const children = await target.getChildren(sectionItem);

        expect(children).toEqual([]);
        expect(vscode.window.showErrorMessage).toHaveBeenCalledWith(expect.stringContaining('Failed to load configuration'));
    });

    test('getChildren(section-auth) handles CLI error gracefully', async () => {
        shellMock.setCommandError('auth list', 'CLI Error');

        const sectionItem = new ConfigurationTreeItem('Auth', 'section-auth', vscode.TreeItemCollapsibleState.Collapsed);
        const children = await target.getChildren(sectionItem);

        expect(children).toEqual([]);
        expect(vscode.window.showErrorMessage).toHaveBeenCalledWith(expect.stringContaining('Failed to load configuration'));
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
});
