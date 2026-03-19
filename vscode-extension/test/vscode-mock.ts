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
    withProgress: jest.fn().mockImplementation(async (_options, task) => {
        return await task({ report: jest.fn() }, { isCancellationRequested: false, onCancellationRequested: jest.fn() });
    })
};

// Mock commands
export const commands = {
    registerCommand: jest.fn()
};

// Mock languages
export const languages = {
    registerDefinitionProvider: jest.fn(),
    registerHoverProvider: jest.fn(),
    registerCompletionItemProvider: jest.fn(),
    registerSignatureHelpProvider: jest.fn(),
    registerReferenceProvider: jest.fn(),
    registerRenameProvider: jest.fn(),
    createDiagnosticCollection: jest.fn().mockReturnValue({
        clear: jest.fn(),
        set: jest.fn(),
        delete: jest.fn(),
        has: jest.fn(),
        forEach: jest.fn(),
        dispose: jest.fn()
    })
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
    openTextDocument: jest.fn(),
    findFiles: jest.fn().mockResolvedValue([])
};

// Mock ViewColumn
export enum ViewColumn {
    One = 1,
    Two = 2
}

export class WorkspaceEdit {
    readonly edits: Array<{ uri: any; range: Range; newText: string }> = [];

    replace(uri: any, range: Range, newText: string): void {
        this.edits.push({ uri, range, newText });
    }

    get size(): number {
        return this.edits.length;
    }
}

export enum CompletionItemKind {
    Text = 0,
    Method = 1,
    Function = 2,
    Constructor = 3,
    Field = 4,
    Variable = 5,
    Class = 6,
    Interface = 7,
    Module = 8,
    Property = 9,
    Unit = 10,
    Value = 11,
    Enum = 12,
    Keyword = 13,
    Snippet = 14,
    Color = 15,
    File = 16,
    Reference = 17,
    Folder = 18,
    EnumMember = 19
}

export class SnippetString {
    constructor(public value: string) {}
}

export class CompletionItem {
    detail?: string;
    documentation?: any;
    insertText?: string | SnippetString;
    commitCharacters?: string[];
    range?: Range;
    command?: any;
    sortText?: string;
    constructor(public label: string, public kind?: CompletionItemKind) {}
}

export class Location {
    public range: Range;
    constructor(public uri: any, rangeOrPosition: Range | Position) {
        this.range = rangeOrPosition instanceof Position
            ? new Range(rangeOrPosition, rangeOrPosition)
            : rangeOrPosition;
    }
}

export class MarkdownString {
    value = '';
    appendMarkdown(s: string) { this.value += s; return this; }
    appendCodeblock(s: string, _lang?: string) { this.value += s; return this; }
}

export class Hover {
    constructor(public contents: any) {}
}

export class SignatureHelp {
    signatures: SignatureInformation[] = [];
    activeSignature = 0;
    activeParameter = 0;
}

export class SignatureInformation {
    parameters: ParameterInformation[] = [];
    documentation?: any;
    constructor(public label: string) {}
}

export class ParameterInformation {
    documentation?: any;
    constructor(public label: string | [number, number]) {}
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
