import * as vscode from 'vscode';
import * as rqClient from '../rqClient';

const LINT_SOURCE = 'rq lint';
const INVALID_RULE = 'invalid_lint_suppression';
const UNUSED_RULE = 'unused_lint_suppression';

const SCOPES: Array<{ scope: rqClient.SuppressionScope; label: string }> = [
    { scope: 'line', label: 'on this line' },
    { scope: 'statement', label: 'for this statement' },
    { scope: 'file', label: 'for this file' }
];

function toRange(edit: rqClient.SourceEdit): vscode.Range {
    return new vscode.Range(edit.start_line - 1, edit.start_column - 1, edit.end_line - 1, edit.end_column - 1);
}

function quickFix(document: vscode.TextDocument, diagnostic: vscode.Diagnostic, title: string, edit: rqClient.SourceEdit): vscode.CodeAction {
    const action = new vscode.CodeAction(title, vscode.CodeActionKind.QuickFix);
    action.diagnostics = [diagnostic];
    action.edit = new vscode.WorkspaceEdit();
    action.edit.replace(document.uri, toRange(edit), edit.new_text);
    return action;
}

async function suppressActions(document: vscode.TextDocument, diagnostic: vscode.Diagnostic, rule: string): Promise<vscode.CodeAction[]> {
    const source = document.getText();
    const line = diagnostic.range.start.line + 1;
    const column = diagnostic.range.start.character + 1;
    const actions: vscode.CodeAction[] = [];
    for (const { scope, label } of SCOPES) {
        const edit = await rqClient.suppressionEdit(source, rule, line, column, scope);
        if (edit) {
            actions.push(quickFix(document, diagnostic, `Suppress ${rule} ${label}`, edit));
        }
    }
    return actions;
}

async function removeUnusedAction(document: vscode.TextDocument, diagnostic: vscode.Diagnostic): Promise<vscode.CodeAction[]> {
    const edit = await rqClient.unusedSuppressionRemoval(
        document.getText(),
        diagnostic.range.start.line + 1,
        diagnostic.range.start.character + 1
    );
    if (!edit) {
        return [];
    }
    const action = quickFix(document, diagnostic, 'Remove unused suppression', edit);
    action.isPreferred = true;
    return [action];
}

async function actionsFor(document: vscode.TextDocument, diagnostic: vscode.Diagnostic): Promise<vscode.CodeAction[]> {
    if (diagnostic.source !== LINT_SOURCE || diagnostic.code === undefined) {
        return [];
    }
    const rule = String(diagnostic.code);
    if (rule === UNUSED_RULE) {
        return removeUnusedAction(document, diagnostic);
    }
    if (rule === INVALID_RULE) {
        return [];
    }
    return suppressActions(document, diagnostic, rule);
}

export const codeActionProvider = vscode.languages.registerCodeActionsProvider(
    'rq',
    {
        async provideCodeActions(document: vscode.TextDocument, _range: vscode.Range, context: vscode.CodeActionContext): Promise<vscode.CodeAction[]> {
            const actions = await Promise.all(context.diagnostics.map(diagnostic => actionsFor(document, diagnostic)));
            return actions.flat();
        }
    },
    { providedCodeActionKinds: [vscode.CodeActionKind.QuickFix] }
);
