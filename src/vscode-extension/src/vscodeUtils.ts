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

export type EnvironmentChoice = { environment: string | undefined };

export function pickEnvironment(
    environments: string[],
    currentEnvironment: string | undefined,
    title: string
): Promise<EnvironmentChoice | undefined> {
    const noEnvironment: vscode.QuickPickItem = { label: 'None', description: 'No environment selected' };
    const environmentItems = environments.map(label => ({ label }));
    const activeItem = currentEnvironment === undefined
        ? noEnvironment
        : environmentItems.find(item => item.label === currentEnvironment);

    const quickPick = vscode.window.createQuickPick();
    quickPick.title = title;
    quickPick.placeholder = activeItem
        ? 'Select an environment'
        : `Environment '${currentEnvironment}' no longer exists, select another one`;
    quickPick.items = [noEnvironment, ...environmentItems];
    quickPick.activeItems = activeItem ? [activeItem] : [];

    return new Promise(resolve => {
        quickPick.onDidAccept(() => {
            const [selected] = quickPick.selectedItems;
            resolve(selected && { environment: selected === noEnvironment ? undefined : selected.label });
            quickPick.hide();
        });
        quickPick.onDidHide(() => {
            resolve(undefined);
            quickPick.dispose();
        });
        quickPick.show();
    });
}
