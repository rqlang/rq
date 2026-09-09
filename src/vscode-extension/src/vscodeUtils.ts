import * as vscode from 'vscode';

export function applyTreeItemLoading(
    item: vscode.TreeItem,
    loading: boolean,
    originalIcons: WeakMap<vscode.TreeItem, vscode.TreeItem['iconPath']>,
    fireChange: (item: vscode.TreeItem) => void
): void {
    if (loading) {
        if (!originalIcons.has(item)) {
            originalIcons.set(item, item.iconPath);
        }
        item.iconPath = new vscode.ThemeIcon('sync~spin');
    } else {
        item.iconPath = originalIcons.get(item) ?? item.iconPath;
        originalIcons.delete(item);
    }
    fireChange(item);
}
