import * as vscode from 'vscode';
import { RequestExplorerProvider, RequestTreeItem } from '../src/requestExplorer';
import * as cp from 'child_process';
import { ShellMock } from './shell-mock';

jest.mock('child_process');

// Jest will automatically swap 'vscode' with our mock because of jest.config.js
// We don't even need to import it here unless we want to check types

describe('RequestExplorerProvider', () => {
    let target: RequestExplorerProvider;
    let shellMock: ShellMock;

    beforeEach(() => {
        shellMock = new ShellMock();
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

    test('setSelectedEnvironment() updates environment and triggers refresh', () => {
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

    test('getChildren() calls CLI and returns grouped items', async () => {
        const mockOutput = [
            { name: 'req1', endpoint: 'GET /api', file: '/root/req1.http' },
            { name: 'req2', endpoint: null, file: '/root/req2.http' }
        ];

        shellMock.setCommandOutput('request list', mockOutput);

        const children = await target.getChildren();

        // Expect: Environment item + 1 endpoint group + 1 top-level request
        // Environment item is always first
        expect(children.length).toBe(3);
        expect(children[0].contextValue).toBe('environment-info');

        // Check for endpoint group
        const endpointItem = children.find(c => c.label === 'GET /api');
        expect(endpointItem).toBeDefined();
        expect(endpointItem?.contextValue).toBe('endpoint');

        // Check for top-level request
        const reqItem = children.find(c => c.label === 'req2');
        expect(reqItem).toBeDefined();
        expect(reqItem?.contextValue).toBe('request');

        expect(cp.spawn).toHaveBeenCalled();
    });

    test('getChildren() handles CLI errors gracefully', async () => {
        const consoleErrorSpy = jest.spyOn(console, 'error').mockImplementation(() => { });

        try {
            shellMock.setCommandError('request list', 'CLI Error');

            const children = await target.getChildren();

            // Should return Environment item + Error item
            expect(children.length).toBe(2);
            expect(children[0].contextValue).toBe('environment-info');
            expect(children[1].contextValue).toBe('error');
            expect(children[1].label).toBe('Error loading requests');

            expect(vscode.window.showErrorMessage).toHaveBeenCalledWith(expect.stringContaining('Failed to list requests'));
        } finally {
            consoleErrorSpy.mockRestore();
        }
    });

    test('getChildren() shows parse errors item when CLI reports warnings', async () => {
        const consoleWarnSpy = jest.spyOn(console, 'warn').mockImplementation(() => { });

        try {
            const mockOutput = [
                { name: 'req1', endpoint: null, file: '/root/req1.http' }
            ];
            const mockStderr = 'Warning: Failed to parse file1.rq: Syntax error\nWarning: Failed to parse file2.rq: Syntax error';

            shellMock.setCommandSuccessWithStderr('request list', mockOutput, mockStderr);

            const children = await target.getChildren();

            // Expect: Environment item + Parse Errors item + 1 request
            expect(children.length).toBe(3);

            expect(children[0].contextValue).toBe('environment-info');

            const errorItem = children[1];
            expect(errorItem.contextValue).toBe('error');
            expect(errorItem.label).toContain('Parse Errors (2)');
            expect(errorItem.tooltip).toContain('file1.rq');
            expect(errorItem.tooltip).toContain('file2.rq');

            const reqItem = children[2];
            expect(reqItem.label).toBe('req1');
        } finally {
            consoleWarnSpy.mockRestore();
        }
    });

    test('getChildren() handles files outside workspace root gracefully', async () => {
        const mockOutput = [
            { name: 'req1', endpoint: null, file: '/outside/req1.http' }
        ];

        shellMock.setCommandOutput('request list', mockOutput);

        const children = await target.getChildren();

        // Expect: Environment item + 1 request (at root level, not in .. folder)
        expect(children.length).toBe(2);
        expect(children[0].contextValue).toBe('environment-info');

        const reqItem = children[1];
        expect(reqItem.label).toBe('req1');
        expect(reqItem.contextValue).toBe('request');
    });
});
