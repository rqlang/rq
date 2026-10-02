jest.mock('../../src/rqClient');

import * as vscode from 'vscode';
import * as rqClient from '../../src/rqClient';
import '../../src/language/codeActionProvider';

const SOURCE = 'ep widgets("http://localhost:8080/widgets") {\n    rq list("");\n}\n';

const document = {
    uri: { fsPath: '/workspace/widgets.rq' },
    getText: () => SOURCE
};

const edit: rqClient.SourceEdit = { start_line: 2, start_column: 17, end_line: 2, end_column: 17, new_text: ' // rq-lint-ignore empty_url_string' };

type ProvideCodeActions = (doc: typeof document, range: vscode.Range, context: { diagnostics: vscode.Diagnostic[] }) => Promise<vscode.CodeAction[]>;

let provideCodeActions: ProvideCodeActions;

function lintDiagnostic(rule: string, source = 'rq lint'): vscode.Diagnostic {
    const diagnostic = new vscode.Diagnostic(new vscode.Range(1, 4, 1, 16), 'message', vscode.DiagnosticSeverity.Warning);
    diagnostic.source = source;
    diagnostic.code = rule;
    return diagnostic;
}

async function actionsFor(diagnostic: vscode.Diagnostic): Promise<vscode.CodeAction[]> {
    return provideCodeActions(document, diagnostic.range, { diagnostics: [diagnostic] });
}

beforeAll(() => {
    const calls = (vscode.languages.registerCodeActionsProvider as jest.Mock).mock.calls;
    provideCodeActions = calls[calls.length - 1][1].provideCodeActions;
});

beforeEach(() => {
    jest.clearAllMocks();
    (rqClient.suppressionEdit as jest.Mock).mockResolvedValue(edit);
    (rqClient.unusedSuppressionRemoval as jest.Mock).mockResolvedValue(edit);
});

describe('suppress quick fixes', () => {
    test('offers one action per scope the library can suppress', async () => {
        const target = await actionsFor(lintDiagnostic('empty_url_string'));

        expect(target.map(a => a.title)).toEqual([
            'Suppress empty_url_string on this line',
            'Suppress empty_url_string for this statement',
            'Suppress empty_url_string for this file'
        ]);
    });

    test('skips a scope for which the library returns no edit', async () => {
        (rqClient.suppressionEdit as jest.Mock).mockImplementation(async (_s, _r, _l, _c, scope) => scope === 'file' ? null : edit);

        const target = await actionsFor(lintDiagnostic('hardcoded_secret'));

        expect(target.map(a => a.title)).not.toContain('Suppress hardcoded_secret for this file');
    });

    test('passes the diagnostic position as 1-based line and column', async () => {
        await actionsFor(lintDiagnostic('empty_url_string'));

        expect(rqClient.suppressionEdit).toHaveBeenCalledWith(SOURCE, 'empty_url_string', 2, 5, 'line');
    });

    test('turns the library edit into a 0-based workspace edit', async () => {
        const [target] = await actionsFor(lintDiagnostic('empty_url_string'));

        const [change] = (target.edit as unknown as { edits: Array<{ range: vscode.Range; newText: string }> }).edits;
        expect([change.range.start.line, change.range.start.character, change.newText])
            .toEqual([1, 16, ' // rq-lint-ignore empty_url_string']);
    });

    test('marks every action as a quick fix for its diagnostic', async () => {
        const diagnostic = lintDiagnostic('empty_url_string');

        const target = await actionsFor(diagnostic);

        expect(target.every(a => a.kind === vscode.CodeActionKind.QuickFix && a.diagnostics?.[0] === diagnostic)).toBe(true);
    });

    test('ignores diagnostics that do not come from the linter', async () => {
        const target = await actionsFor(lintDiagnostic('empty_url_string', 'rq'));

        expect(target).toEqual([]);
    });

    test('offers nothing for an invalid suppression', async () => {
        const target = await actionsFor(lintDiagnostic('invalid_lint_suppression'));

        expect(target).toEqual([]);
    });
});

describe('unused suppression quick fix', () => {
    test('offers to remove an unused suppression', async () => {
        const target = await actionsFor(lintDiagnostic('unused_lint_suppression'));

        expect(target.map(a => a.title)).toEqual(['Remove unused suppression']);
    });

    test('offers no suppression of the unused suppression itself', async () => {
        await actionsFor(lintDiagnostic('unused_lint_suppression'));

        expect(rqClient.suppressionEdit).not.toHaveBeenCalled();
    });
});
