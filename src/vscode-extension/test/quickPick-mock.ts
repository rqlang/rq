import * as vscode from 'vscode';

export type QuickPickAction = { accept: string } | 'acceptActive' | 'cancel';

export type FakeQuickPick = {
    title: string;
    placeholder: string;
    items: { label: string }[];
    activeItems: { label: string }[];
    selectedItems: { label: string }[];
};

export function mockQuickPick(action: QuickPickAction): FakeQuickPick {
    const acceptListeners: (() => void)[] = [];
    const hideListeners: (() => void)[] = [];
    const quickPick = {
        title: '',
        placeholder: '',
        items: [] as { label: string }[],
        activeItems: [] as { label: string }[],
        selectedItems: [] as { label: string }[],
        onDidAccept: (listener: () => void) => { acceptListeners.push(listener); return { dispose: jest.fn() }; },
        onDidHide: (listener: () => void) => { hideListeners.push(listener); return { dispose: jest.fn() }; },
        hide: () => hideListeners.forEach(listener => listener()),
        dispose: jest.fn(),
        show: () => {
            if (action === 'cancel') {
                quickPick.hide();
                return;
            }
            quickPick.selectedItems = action === 'acceptActive'
                ? quickPick.activeItems.slice(0, 1)
                : quickPick.items.filter(item => item.label === action.accept);
            acceptListeners.forEach(listener => listener());
        }
    };
    (vscode.window.createQuickPick as jest.Mock).mockReturnValueOnce(quickPick);
    return quickPick;
}
