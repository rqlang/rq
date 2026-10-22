import * as vscode from 'vscode';
import { ResultsPanels, readResultsTabMode } from '../src/ui/resultsPanels';

function createMockPanel(_viewType: string, title: string) {
    const panel: any = {
        title,
        webview: {
            html: '',
            onDidReceiveMessage: jest.fn(),
            postMessage: jest.fn()
        },
        reveal: jest.fn(),
        onDidDispose: jest.fn((listener: () => void) => { panel.dispose = listener; })
    };
    return panel;
}

describe('ResultsPanels', () => {
    let target: ResultsPanels<string>;
    let onMessage: jest.Mock;

    beforeEach(() => {
        jest.clearAllMocks();
        (vscode.window.createWebviewPanel as jest.Mock).mockImplementation(createMockPanel);
        onMessage = jest.fn();
        target = new ResultsPanels<string>(onMessage);
    });

    test('single mode reuses one panel for different requests', () => {
        const first = target.show(target.sessionFor('single', 'a', 'a.rq'), 'a');
        const second = target.show(target.sessionFor('single', 'b', 'b.rq'), 'b');

        expect(second).toBe(first);
        expect(second.title).toBe('RQ: b');
        expect(second.reveal).toHaveBeenCalledWith(vscode.ViewColumn.One);
        expect(vscode.window.createWebviewPanel).toHaveBeenCalledTimes(1);
    });

    test('perRequest mode reuses the panel of the same request', () => {
        const first = target.show(target.sessionFor('perRequest', 'a', 'a.rq'), 'a');
        const second = target.show(target.sessionFor('perRequest', 'a', 'a.rq'), 'a');

        expect(second).toBe(first);
        expect(vscode.window.createWebviewPanel).toHaveBeenCalledTimes(1);
    });

    test('perRequest mode opens a panel per request', () => {
        const first = target.show(target.sessionFor('perRequest', 'a', 'a.rq'), 'a');
        const second = target.show(target.sessionFor('perRequest', 'b', 'a.rq'), 'b');

        expect(second).not.toBe(first);
        expect(first.title).toBe('RQ: a');
        expect(second.title).toBe('RQ: b');
    });

    test('perRequest mode separates requests with the same name in different files', () => {
        const first = target.show(target.sessionFor('perRequest', 'a', 'one.rq'), 'a');
        const second = target.show(target.sessionFor('perRequest', 'a', 'two.rq'), 'a');

        expect(second).not.toBe(first);
    });

    test('perExecution mode opens a new panel every time', () => {
        const first = target.show(target.sessionFor('perExecution', 'a', 'a.rq'), 'a');
        const second = target.show(target.sessionFor('perExecution', 'a', 'a.rq'), 'a');

        expect(second).not.toBe(first);
        expect(vscode.window.createWebviewPanel).toHaveBeenCalledTimes(2);
    });

    test('perExecution mode adds the execution time to the title', () => {
        const panel = target.show(target.sessionFor('perExecution', 'a', 'a.rq'), 'a');

        expect(panel.title).toMatch(/^RQ: a \(.+\)$/);
    });

    test('showing a perExecution session again reuses its panel', () => {
        const session = target.sessionFor('perExecution', 'a', 'a.rq');
        const first = target.show(session, 'a');
        const second = target.show(session, 'a');

        expect(second).toBe(first);
        expect(vscode.window.createWebviewPanel).toHaveBeenCalledTimes(1);
    });

    test('a closed panel is replaced by a new one for the same request', () => {
        const first: any = target.show(target.sessionFor('perRequest', 'a', 'a.rq'), 'a');
        first.dispose();

        const second = target.show(target.sessionFor('perRequest', 'a', 'a.rq'), 'a');

        expect(second).not.toBe(first);
        expect(vscode.window.createWebviewPanel).toHaveBeenCalledTimes(2);
    });

    test('a session keeps its data until its panel is closed', () => {
        const session = target.sessionFor('perRequest', 'a', 'a.rq');
        session.data = 'stored';
        target.show(session, 'a');

        expect(target.sessionFor('perRequest', 'a', 'a.rq').data).toBe('stored');
    });

    test('messages are delivered with the session of the panel that sent them', () => {
        const session = target.sessionFor('perRequest', 'a', 'a.rq');
        const panel: any = target.show(session, 'a');
        const listener = panel.webview.onDidReceiveMessage.mock.calls[0][0];

        listener({ command: 'runAgain' });

        expect(onMessage).toHaveBeenCalledWith(session, { command: 'runAgain' });
    });
});

describe('readResultsTabMode', () => {
    function configure(value: unknown) {
        (vscode.workspace.getConfiguration as jest.Mock).mockReturnValueOnce({
            get: jest.fn().mockReturnValue(value)
        });
    }

    test('returns the configured mode', () => {
        configure('perExecution');

        expect(readResultsTabMode()).toBe('perExecution');
    });

    test('falls back to single for an unknown value', () => {
        configure('everywhere');

        expect(readResultsTabMode()).toBe('single');
    });
});
