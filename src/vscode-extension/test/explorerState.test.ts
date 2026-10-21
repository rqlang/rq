import * as vscode from 'vscode';
import { RestorableTreeDataProvider, TreeItemIds, trackExplorerState } from '../src/explorerState';
import { FakeMemento, createExplorerState } from './memento-mock';

type FakeView = {
    visible: boolean;
    reveal: jest.Mock;
    expand: vscode.EventEmitter<{ element: vscode.TreeItem }>;
    collapse: vscode.EventEmitter<{ element: vscode.TreeItem }>;
    select: vscode.EventEmitter<{ selection: vscode.TreeItem[] }>;
};

function makeItem(id: string, state = vscode.TreeItemCollapsibleState.None): vscode.TreeItem {
    const item = new vscode.TreeItem(id, state);
    item.id = id;
    return item;
}

function makeView(): FakeView {
    return {
        visible: true,
        reveal: jest.fn().mockResolvedValue(undefined),
        expand: new vscode.EventEmitter(),
        collapse: new vscode.EventEmitter(),
        select: new vscode.EventEmitter(),
    };
}

function asTreeView(view: FakeView): vscode.TreeView<vscode.TreeItem> {
    return {
        get visible() { return view.visible; },
        reveal: view.reveal,
        onDidExpandElement: view.expand.event,
        onDidCollapseElement: view.collapse.event,
        onDidChangeSelection: view.select.event,
    } as unknown as vscode.TreeView<vscode.TreeItem>;
}

function makeProvider(items: vscode.TreeItem[], parents: Map<vscode.TreeItem, vscode.TreeItem> = new Map(), loaded = true) {
    const load = new vscode.EventEmitter<void>();
    const provider = {
        onDidLoad: load.event,
        findItem: (id: string) => items.find(i => i.id === id),
        isLoaded: () => loaded,
        getParent: (element: vscode.TreeItem) => parents.get(element),
        getTreeItem: (element: vscode.TreeItem) => element,
        getChildren: () => items,
    } as RestorableTreeDataProvider<vscode.TreeItem>;
    return { provider, fireLoad: () => load.fire() };
}

describe('ExplorerState', () => {
    test('collapsibleState() returns the fallback for an unknown id', () => {
        const target = createExplorerState();

        expect(target.collapsibleState('a', vscode.TreeItemCollapsibleState.Expanded)).toBe(vscode.TreeItemCollapsibleState.Expanded);
    });

    test('collapsibleState() returns the stored expansion over the fallback', () => {
        const target = createExplorerState();
        target.setExpanded('a', false);
        target.setExpanded('b', true);

        expect(target.collapsibleState('a', vscode.TreeItemCollapsibleState.Expanded)).toBe(vscode.TreeItemCollapsibleState.Collapsed);
        expect(target.collapsibleState('b', vscode.TreeItemCollapsibleState.Collapsed)).toBe(vscode.TreeItemCollapsibleState.Expanded);
    });

    test('keeps every stored value across instances sharing the memento', () => {
        const memento = new FakeMemento();
        const first = createExplorerState(memento);
        first.setEnvironment('prod');
        first.setSelected('request:a');
        first.setExpanded('folder:x', true);

        const target = createExplorerState(memento);

        expect(target.environment).toBe('prod');
        expect(target.selected).toBe('request:a');
        expect(target.collapsibleState('folder:x', vscode.TreeItemCollapsibleState.Collapsed)).toBe(vscode.TreeItemCollapsibleState.Expanded);
    });
});

describe('TreeItemIds', () => {
    test('suffixes an id that was already used', () => {
        const target = new TreeItemIds();

        expect([target.next('a'), target.next('a'), target.next('a'), target.next('b')]).toEqual(['a', 'a~2', 'a~3', 'b']);
    });
});

describe('trackExplorerState', () => {
    test('stores expanded and collapsed elements', () => {
        const state = createExplorerState();
        const view = makeView();
        trackExplorerState(asTreeView(view), makeProvider([]).provider, state);

        view.expand.fire({ element: makeItem('a') });
        view.collapse.fire({ element: makeItem('b') });

        expect(state.collapsibleState('a', vscode.TreeItemCollapsibleState.Collapsed)).toBe(vscode.TreeItemCollapsibleState.Expanded);
        expect(state.collapsibleState('b', vscode.TreeItemCollapsibleState.Expanded)).toBe(vscode.TreeItemCollapsibleState.Collapsed);
    });

    test('stores the selected element', () => {
        const state = createExplorerState();
        const view = makeView();
        trackExplorerState(asTreeView(view), makeProvider([]).provider, state);

        view.select.fire({ selection: [makeItem('a')] });

        expect(state.selected).toBe('a');
    });

    test('reveals the stored selection once it is loaded', () => {
        const state = createExplorerState();
        state.setSelected('a');
        const item = makeItem('a');
        const view = makeView();
        const { provider, fireLoad } = makeProvider([item]);
        trackExplorerState(asTreeView(view), provider, state);

        fireLoad();
        fireLoad();

        expect(view.reveal).toHaveBeenCalledTimes(1);
        expect(view.reveal).toHaveBeenCalledWith(item, { select: true, focus: false });
    });

    test('does not reveal the stored selection inside a collapsed parent', () => {
        const state = createExplorerState();
        state.setSelected('a');
        const item = makeItem('a');
        const parent = makeItem('p', vscode.TreeItemCollapsibleState.Collapsed);
        const view = makeView();
        const { provider, fireLoad } = makeProvider([item, parent], new Map([[item, parent]]));
        trackExplorerState(asTreeView(view), provider, state);

        fireLoad();

        expect(view.reveal).not.toHaveBeenCalled();
    });

    test('does not reveal the stored selection while the view is hidden', () => {
        const state = createExplorerState();
        state.setSelected('a');
        const view = makeView();
        view.visible = false;
        const { provider, fireLoad } = makeProvider([makeItem('a')]);
        trackExplorerState(asTreeView(view), provider, state);

        fireLoad();

        expect(view.reveal).not.toHaveBeenCalled();
    });

    test('does not reveal the stored selection after the user selects another element', () => {
        const state = createExplorerState();
        state.setSelected('a');
        const view = makeView();
        const { provider, fireLoad } = makeProvider([makeItem('a')]);
        trackExplorerState(asTreeView(view), provider, state);

        view.select.fire({ selection: [makeItem('b')] });
        fireLoad();

        expect(view.reveal).not.toHaveBeenCalled();
    });

    test('forgets a stored selection that no longer exists once its part of the tree is loaded', () => {
        const state = createExplorerState();
        state.setSelected('gone');
        const view = makeView();
        const items: vscode.TreeItem[] = [];
        const { provider, fireLoad } = makeProvider(items);
        trackExplorerState(asTreeView(view), provider, state);

        fireLoad();
        items.push(makeItem('gone'));
        fireLoad();

        expect(state.selected).toBeUndefined();
        expect(view.reveal).not.toHaveBeenCalled();
    });

    test('keeps waiting for a stored selection whose part of the tree is not loaded yet', () => {
        const state = createExplorerState();
        state.setSelected('a');
        const view = makeView();
        const items: vscode.TreeItem[] = [];
        const { provider, fireLoad } = makeProvider(items, new Map(), false);
        trackExplorerState(asTreeView(view), provider, state);

        fireLoad();
        items.push(makeItem('a'));
        fireLoad();

        expect(state.selected).toBe('a');
        expect(view.reveal).toHaveBeenCalledTimes(1);
    });

    test('keeps the stored selection when its item disappears in an incomplete load', () => {
        const state = createExplorerState();
        const item = makeItem('a');
        const view = makeView();
        const items: vscode.TreeItem[] = [item];
        const { provider, fireLoad } = makeProvider(items, new Map(), false);
        trackExplorerState(asTreeView(view), provider, state);
        view.select.fire({ selection: [item] });

        items.pop();
        view.select.fire({ selection: [] });
        items.push(item);
        fireLoad();

        expect(state.selected).toBe('a');
        expect(view.reveal).toHaveBeenCalledWith(item, { select: true, focus: false });
    });

    test('forgets the stored selection when the user deselects an item that still exists', () => {
        const state = createExplorerState();
        const item = makeItem('a');
        const view = makeView();
        const { provider } = makeProvider([item], new Map(), false);
        trackExplorerState(asTreeView(view), provider, state);
        view.select.fire({ selection: [item] });

        view.select.fire({ selection: [] });

        expect(state.selected).toBeUndefined();
    });

    test('does not reveal the stored selection inside a parent the user collapsed after it was built', () => {
        const state = createExplorerState();
        state.setSelected('a');
        const item = makeItem('a');
        const parent = makeItem('p', vscode.TreeItemCollapsibleState.Expanded);
        const view = makeView();
        const { provider, fireLoad } = makeProvider([item, parent], new Map([[item, parent]]));
        trackExplorerState(asTreeView(view), provider, state);

        view.collapse.fire({ element: parent });
        fireLoad();

        expect(view.reveal).not.toHaveBeenCalled();
    });
});
