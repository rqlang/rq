import * as vscode from 'vscode';

export const RESULTS_TAB_MODES = ['single', 'perRequest', 'perExecution'] as const;

export type ResultsTabMode = typeof RESULTS_TAB_MODES[number];

export interface ResultsSession<T> {
    readonly key: string;
    readonly timestamped: boolean;
    panel: vscode.WebviewPanel | undefined;
    data: T | undefined;
}

export interface ResultsMessage {
    command?: string;
    text?: unknown;
}

export type ResultsMessageHandler<T> = (session: ResultsSession<T>, message: ResultsMessage) => void;

export function readResultsTabMode(): ResultsTabMode {
    const configured = vscode.workspace.getConfiguration('rq').get<string>('results.tabMode', 'single');
    return RESULTS_TAB_MODES.find(mode => mode === configured) ?? 'single';
}

export class ResultsPanels<T> {
    private readonly sessions = new Map<string, ResultsSession<T>>();
    private executionCount = 0;

    constructor(private readonly onMessage: ResultsMessageHandler<T>) {}

    public sessionFor(mode: ResultsTabMode, requestName: string, sourceFile: string | undefined): ResultsSession<T> {
        const key = this.keyFor(mode, requestName, sourceFile);
        return this.sessions.get(key) ?? {
            key,
            timestamped: mode === 'perExecution',
            panel: undefined,
            data: undefined
        };
    }

    public show(session: ResultsSession<T>, requestName: string): vscode.WebviewPanel {
        const title = this.titleFor(session, requestName);
        if (session.panel) {
            session.panel.title = title;
            session.panel.reveal(vscode.ViewColumn.One);
            return session.panel;
        }
        session.panel = this.createPanel(session, title);
        this.sessions.set(session.key, session);
        return session.panel;
    }

    private keyFor(mode: ResultsTabMode, requestName: string, sourceFile: string | undefined): string {
        switch (mode) {
            case 'perRequest':
                return `request:${sourceFile ?? ''}:${requestName}`;
            case 'perExecution':
                this.executionCount += 1;
                return `execution:${this.executionCount}`;
            default:
                return 'single';
        }
    }

    private titleFor(session: ResultsSession<T>, requestName: string): string {
        return session.timestamped
            ? `RQ: ${requestName} (${new Date().toLocaleTimeString()})`
            : `RQ: ${requestName}`;
    }

    private createPanel(session: ResultsSession<T>, title: string): vscode.WebviewPanel {
        const panel = vscode.window.createWebviewPanel(
            'rqRequestResult',
            title,
            vscode.ViewColumn.One,
            { enableScripts: true, retainContextWhenHidden: true }
        );
        const subscriptions: vscode.Disposable[] = [];
        panel.webview.onDidReceiveMessage((message: ResultsMessage) => this.onMessage(session, message), undefined, subscriptions);
        panel.onDidDispose(() => {
            subscriptions.forEach(subscription => subscription.dispose());
            session.panel = undefined;
            if (this.sessions.get(session.key) === session) {
                this.sessions.delete(session.key);
            }
        });
        return panel;
    }
}
