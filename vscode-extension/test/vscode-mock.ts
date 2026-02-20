// Mock EventEmitter
export class EventEmitter {
    private listeners: ((e: any) => any)[] = [];
    
    event = (listener: (e: any) => any) => {
        this.listeners.push(listener);
        return { dispose: () => {} };
    };

    fire(data?: any) {
        this.listeners.forEach(l => l(data));
    }
}

// Mock TreeItem
export class TreeItem {
    constructor(public label: string, public collapsibleState?: any) {}
}

// Mock ThemeIcon
export class ThemeIcon {
    constructor(public id: string) {}
}

// Mock Enums
export enum TreeItemCollapsibleState {
    None = 0,
    Collapsed = 1,
    Expanded = 2
}

// Mock window
export const window = {
    showInformationMessage: jest.fn(),
    showErrorMessage: jest.fn(),
    showWarningMessage: jest.fn(),
    showQuickPick: jest.fn(),
    showInputBox: jest.fn(),
    createOutputChannel: jest.fn(),
    createWebviewPanel: jest.fn(),
    registerUriHandler: jest.fn(),
    showTextDocument: jest.fn(),
    withProgress: jest.fn()
};

// Mock commands
export const commands = {
    registerCommand: jest.fn()
};

// Mock env
export const env = {
    clipboard: {
        writeText: jest.fn()
    },
    openExternal: jest.fn()
};

// Mock workspace
export const workspace = {
    getConfiguration: jest.fn().mockReturnValue({
        get: jest.fn().mockReturnValue(false)
    }),
    workspaceFolders: [],
    openTextDocument: jest.fn()
};

// Mock ViewColumn
export enum ViewColumn {
    One = 1,
    Two = 2
}

export class Position {
    constructor(public line: number, public character: number) {}
}

export class Range {
    constructor(public start: Position, public end: Position) {}
}

export class Selection extends Range {
    constructor(anchor: Position, active: Position) {
        super(anchor, active);
    }
}

export enum TextEditorRevealType {
    Default = 0,
    InCenter = 1,
    InCenterIfOutsideViewport = 2,
    AtTop = 3
}

export enum ProgressLocation {
    SourceControl = 1,
    Window = 10,
    Notification = 15
}

export enum ExtensionMode {
    Production = 1,
    Development = 2,
    Test = 3
}

// Mock other used types
export const Uri = {
    file: (path: string) => ({ fsPath: path }),
    parse: (value: string) => {
        try {
            const url = new URL(value);
            return {
                scheme: url.protocol.replace(':', ''),
                authority: url.host,
                path: url.pathname,
                query: url.search.substring(1),
                fragment: url.hash.substring(1),
                fsPath: url.pathname,
                toString: () => value
            };
        } catch (e) {
            return {
                scheme: '',
                authority: '',
                path: value,
                query: '',
                fragment: '',
                fsPath: value,
                toString: () => value
            };
        }
    }
};
