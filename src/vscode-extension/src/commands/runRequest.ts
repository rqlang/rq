import * as vscode from 'vscode';
import * as rqClient from '../rqClient';
import { RequestExplorerProvider, RequestTreeItem } from '../requestExplorer';
import { performOAuth2Flow } from '../auth';
import { getWebviewContent, getErrorWebviewContent } from '../ui/webviewGenerator';
import { ResultsMessage, ResultsPanels, ResultsSession, readResultsTabMode } from '../ui/resultsPanels';
import { Logger } from '../logger';
import { CANCELLED_MESSAGE } from '../utils';

const RUNNING_CONTEXT_KEY = 'rq.requestRunning';

interface RequestExecutionContext {
    requestName: string;
    sourceDirectory: string | undefined;
    sourceFile: string | undefined;
    environment: string | undefined;
    variables: Record<string, string> | undefined;
}

interface RequestRun {
    executionContext: RequestExecutionContext;
    treeItem: RequestTreeItem | undefined;
}

interface ResultsData {
    run: RequestRun;
    body: string | undefined;
}

type RequestResultsSession = ResultsSession<ResultsData>;

interface ActiveExecution {
    run: RequestRun;
    cancellationSource: vscode.CancellationTokenSource;
    session: RequestResultsSession;
}

export class RequestRunner {
    private readonly resultsPanels = new ResultsPanels<ResultsData>((session, message) => this.handlePanelMessage(session, message));
    private logger!: Logger;
    private provider: RequestExplorerProvider | undefined;
    private activeExecution: ActiveExecution | undefined;

    constructor(
        private context: vscode.ExtensionContext,
        private outputChannel: vscode.OutputChannel
    ) {}

    public registerCommands(provider: RequestExplorerProvider) {
        this.provider = provider;
        this.context.subscriptions.push(
            vscode.commands.registerCommand('rq.runRequest', (item) => this.runRequest(item, provider)),
            vscode.commands.registerCommand('rq.runRequestWithVariables', (item) => this.runRequestWithVariables(item, provider)),
            vscode.commands.registerCommand('rq.cancelRequest', () => this.cancelActiveExecution())
        );
    }

    public isExecuting(): boolean {
        return this.activeExecution !== undefined;
    }

    public async runRequest(requestItem: RequestTreeItem, provider: RequestExplorerProvider) {
        await this.runFromExplorer(requestItem, provider, false);
    }

    public async runRequestWithVariables(requestItem: RequestTreeItem, provider: RequestExplorerProvider) {
        await this.runFromExplorer(requestItem, provider, true);
    }

    public cancelActiveExecution() {
        if (!this.activeExecution) {
            return;
        }
        this.logger?.log(`Cancelling request: ${this.activeExecution.run.executionContext.requestName}`);
        this.activeExecution.cancellationSource.cancel();
    }

    private async runFromExplorer(requestItem: RequestTreeItem, provider: RequestExplorerProvider, promptForVariables: boolean) {
        if (!requestItem.request) {
            vscode.window.showErrorMessage('Cannot run: No request information available');
            return;
        }
        if (!this.ensureNoActiveExecution()) {
            return;
        }

        this.provider = provider;
        const { name: requestName, file: sourceFile } = requestItem.request;
        const session = this.resultsPanels.sessionFor(readResultsTabMode(), requestName, sourceFile);
        await this.execute(session, { requestName, sourceFile, variables: undefined }, requestItem, promptForVariables);
    }

    private async runAgain(session: RequestResultsSession) {
        if (!session.data) {
            return;
        }
        if (!this.ensureNoActiveExecution()) {
            return;
        }

        const { executionContext, treeItem } = session.data.run;
        await this.execute(session, executionContext, treeItem, false);
    }

    private async execute(
        session: RequestResultsSession,
        request: Pick<RequestExecutionContext, 'requestName' | 'sourceFile' | 'variables'>,
        treeItem: RequestTreeItem | undefined,
        promptForVariables: boolean
    ) {
        this.logger = Logger.init(this.outputChannel);
        const executionContext: RequestExecutionContext = {
            ...request,
            sourceDirectory: this.getSourceDirectory(),
            environment: this.provider?.getSelectedEnvironment()
        };
        const execution = this.beginExecution({ executionContext, treeItem }, session);
        try {
            this.logger.log(`Selected environment: ${executionContext.environment || '(none)'}`);

            const variables: Record<string, string> = { ...request.variables };
            if (promptForVariables) {
                await this.collectUserVariables(variables);
                this.throwIfCancelled();
            }
            await this.collectRequiredVariables(executionContext, variables);
            this.throwIfCancelled();
            executionContext.variables = Object.keys(variables).length > 0 ? variables : undefined;

            await this.executeRequestLogic(execution);
        } catch (error) {
            await this.handleError(error);
        } finally {
            this.endExecution();
        }
    }

    private getSourceDirectory(): string | undefined {
        return vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    }

    private ensureNoActiveExecution(): boolean {
        if (!this.activeExecution) {
            return true;
        }
        vscode.window.showWarningMessage(
            `Request '${this.activeExecution.run.executionContext.requestName}' is already running. Cancel it before starting another one.`
        );
        return false;
    }

    private beginExecution(run: RequestRun, session: RequestResultsSession): ActiveExecution {
        const execution: ActiveExecution = {
            run,
            cancellationSource: new vscode.CancellationTokenSource(),
            session
        };
        this.activeExecution = execution;
        if (run.treeItem) {
            this.provider?.setItemRunning(run.treeItem, true);
        }
        vscode.commands.executeCommand('setContext', RUNNING_CONTEXT_KEY, true);
        this.postExecutionState(session, true);
        return execution;
    }

    private endExecution() {
        const execution = this.activeExecution;
        this.activeExecution = undefined;
        if (execution?.run.treeItem) {
            this.provider?.setItemRunning(execution.run.treeItem, false);
        }
        execution?.cancellationSource.dispose();
        vscode.commands.executeCommand('setContext', RUNNING_CONTEXT_KEY, false);
        if (execution) {
            this.postExecutionState(execution.session, false);
        }
    }

    private throwIfCancelled() {
        if (this.activeExecution?.cancellationSource.token.isCancellationRequested) {
            throw new Error(CANCELLED_MESSAGE);
        }
    }

    private postExecutionState(session: RequestResultsSession, running: boolean) {
        session.panel?.webview.postMessage({ command: 'executionState', running });
    }

    private async acquireInteractiveToken(auth: rqClient.PreparedAuth): Promise<string> {
        if (auth.auth_type !== 'oauth2_authorization_code' && auth.auth_type !== 'oauth2_implicit') {
            throw new Error(`Auth configuration '${auth.name}' has unsupported type '${auth.auth_type}'`);
        }
        this.logger.debug(`Detected OAuth2 auth: ${auth.name} (${auth.auth_type})`);
        this.logger.log(`Performing OAuth2 flow...`);
        try {
            const accessToken = await performOAuth2Flow(auth, this.context, this.outputChannel);
            this.logger.log(`OAuth2 token obtained, injecting as auth_token variable`);
            return accessToken;
        } catch (error) {
            const errorMessage = error instanceof Error ? error.message : String(error);
            if (errorMessage.startsWith('Authentication cancelled')) {
                throw new Error(CANCELLED_MESSAGE);
            }
            throw error;
        }
    }

    private async collectRequiredVariables(executionContext: RequestExecutionContext, variables: Record<string, string>) {
        const { requestName, sourceDirectory, sourceFile, environment } = executionContext;
        let requestDetails: rqClient.RequestShowOutput;
        try {
            requestDetails = await rqClient.showRequest(requestName, sourceDirectory, environment, false, false, sourceFile);
        } catch {
            return;
        }

        const missing = requestDetails.requiredVariables.filter(name => !(name in variables));
        for (const varName of missing) {
            const value = await vscode.window.showInputBox({
                prompt: `Enter value for required variable: ${varName}`,
                placeHolder: varName,
                validateInput: (v) => v.trim() ? null : 'Value cannot be empty'
            });

            if (value === undefined) {
                throw new Error(CANCELLED_MESSAGE);
            }

            variables[varName] = value;
        }
    }

    private async collectUserVariables(variables: Record<string, string>) {
        while (true) {
            const count = Object.keys(variables).length;
            const prompt = count === 0
                ? 'Enter variable (variable=value), or leave empty to execute without variables'
                : `${count} variable(s) set. Enter another (variable=value), or leave empty to execute`;

            const variableInput = await vscode.window.showInputBox({
                prompt,
                placeHolder: 'e.g., user_id=123',
                validateInput: this.validateVariableInput
            });

            if (variableInput === undefined) {
                throw new Error(CANCELLED_MESSAGE);
            }

            if (!variableInput) {
                break;
            }

            const eqIndex = variableInput.indexOf('=');
            const varName = variableInput.slice(0, eqIndex).trim();
            const varValue = variableInput.slice(eqIndex + 1).trim();
            variables[varName] = varValue;
        }
    }

    private validateVariableInput(value: string): string | null {
        if (!value) { return null; }
        const eqIndex = value.indexOf('=');
        if (eqIndex === -1) { return 'Variable must be in format: variable=value'; }
        const name = value.slice(0, eqIndex).trim();
        const val = value.slice(eqIndex + 1).trim();
        if (!name || !val) { return 'Invalid format. Use: variable=value'; }
        if (name.includes('"') || val.includes('"')) { return 'Double quotes (") are not allowed in variable names or values'; }
        return null;
    }

    private async executeRequestLogic(execution: ActiveExecution) {
        const { requestName, sourceDirectory, sourceFile, environment, variables } = execution.run.executionContext;

        await vscode.window.withProgress({
            location: vscode.ProgressLocation.Notification,
            title: `Running request: ${requestName}`,
            cancellable: true
        }, async (_progress, token) => {
            const subscription = token.onCancellationRequested(() => this.cancelActiveExecution());
            try {
                const result = await rqClient.executeRequest({
                    requestName,
                    sourceDirectory,
                    sourceFile,
                    environment,
                    variables,
                    cancellation: execution.cancellationSource.token,
                    acquireToken: (auth) => this.acquireInteractiveToken(auth)
                });

                if (result.results.length > 0) {
                    await this.handleSuccessfulExecution(execution, result);
                } else {
                    await this.handleFailedExecution(execution, result);
                }
            } finally {
                subscription.dispose();
            }
        });
    }

    private async handleSuccessfulExecution(execution: ActiveExecution, result: rqClient.ExecuteRequestResult) {
        const { requestName, variables } = execution.run.executionContext;
        this.logOutput(`Request: ${requestName}`, result.results[0], variables);

        if (result.stderr) {
            this.logger.log(`STDERR output:\n${result.stderr}`);
        }

        const { session } = execution;
        const panel = this.resultsPanels.show(session, requestName);
        session.data = { run: execution.run, body: result.results[0].body };
        panel.webview.html = await getWebviewContent(this.context, result.results[0]);
    }

    private async handleFailedExecution(execution: ActiveExecution, result: rqClient.ExecuteRequestResult) {
        const { requestName, variables } = execution.run.executionContext;
        const fullError = result.stderr
            ? this.cleanErrorMessage(result.stderr)
            : `No results returned for request: ${requestName}`;

        this.logger.log(`\n${'='.repeat(80)}`);
        this.logger.log(`Request Failed: ${requestName}`);
        if (variables) { this.logger.debug(`Variables: ${JSON.stringify(variables)}`); }
        this.logger.log(`Time: ${new Date().toISOString()}`);
        this.logger.log(`${'='.repeat(80)}`);
        this.logger.log(fullError);
        this.logger.log(`${'='.repeat(80)}\n`);

        await this.showErrorPanel(execution, fullError);
    }

    private handlePanelMessage(session: RequestResultsSession, message: ResultsMessage) {
        if (message.command === 'runAgain') {
            this.runAgain(session);
        }
        if (message.command === 'cancel' && this.activeExecution?.session === session) {
            this.cancelActiveExecution();
        }
        if (message.command === 'copyBody' && session.data?.body !== undefined) {
            vscode.env.clipboard.writeText(session.data.body);
            vscode.window.showInformationMessage('Body copied to clipboard');
        }
        if (message.command === 'copy' && typeof message.text === 'string') {
            vscode.env.clipboard.writeText(message.text);
            vscode.window.showInformationMessage('Copied to clipboard');
        }
    }

    private logOutput(title: string, result: rqClient.RequestExecutionResult, variables?: Record<string, string>) {
        this.logger.log(`\n${'='.repeat(80)}`);
        this.logger.log(title);
        if (variables) { this.logger.debug(`Variables: ${JSON.stringify(variables)}`); }
        this.logger.log(`Time: ${new Date().toISOString()}`);
        this.logger.log(`${'='.repeat(80)}`);
        this.logger.log(`${result.method} ${result.url} → ${result.status} (${result.elapsed_ms}ms)`);
        this.logger.debug(`\nRequest Headers:`);
        for (const [key, value] of Object.entries(result.request_headers)) {
            const display = key.toLowerCase() === 'authorization' ? Logger.redactAuthValue(value) : value;
            this.logger.debug(`  ${key}: ${display}`);
        }
        if (result.request_body) { this.logger.debug(`\nRequest Body:\n${result.request_body}`); }
        this.logger.debug(`\nResponse Headers:`);
        for (const [key, value] of Object.entries(result.response_headers)) {
            this.logger.debug(`  ${key}: ${value}`);
        }
        this.logger.log(`${'='.repeat(80)}\n`);
    }

    private async handleError(error: unknown) {
        let errorMessage: string;
        const asAggregate = error as { errors?: unknown[] };
        const unwrapped = Array.isArray(asAggregate?.errors) && asAggregate.errors.length > 0
            ? asAggregate.errors[0]
            : error;
        if (unwrapped instanceof Error) {
            errorMessage = unwrapped.message || unwrapped.toString();
            const code = (unwrapped as NodeJS.ErrnoException).code;
            if (!errorMessage || errorMessage === '[object Error]') {
                errorMessage = code ?? 'Unknown error';
            } else if (code && !errorMessage.includes(code)) {
                errorMessage = `${code}: ${errorMessage}`;
            }
        } else {
            errorMessage = String(unwrapped) || 'Unknown error';
        }
        if (errorMessage === CANCELLED_MESSAGE) {
            this.logger?.log(`\n${CANCELLED_MESSAGE}\n`);
            return;
        }
        this.logger.log(`\nERROR: ${errorMessage}\n`);
        if (this.activeExecution) {
            await this.showErrorPanel(this.activeExecution, errorMessage);
        }
    }

    private async showErrorPanel(execution: ActiveExecution, errorMessage: string) {
        const { session, run } = execution;
        const { requestName, sourceDirectory, sourceFile, environment } = run.executionContext;
        const panel = this.resultsPanels.show(session, requestName);
        session.data = { run, body: undefined };

        let requestDetails: rqClient.RequestShowOutput | undefined;
        try {
            requestDetails = await rqClient.showRequest(
                requestName,
                sourceDirectory,
                environment,
                true,
                false,
                sourceFile
            );
        } catch {
            // best-effort: show error panel without request details
        }

        panel.webview.html = await getErrorWebviewContent(this.context, requestName, errorMessage, requestDetails);
    }

    private cleanErrorMessage(stderr: string): string {
        const lines = stderr.split('\n')
            .map(l => l.trim())
            .filter(l => {
                if (l.length === 0) { return false; }
                if (l.startsWith('Warning:')) { return false; }
                if (l.startsWith('Finished ')) { return false; }
                if (l.startsWith('Running `')) { return false; }
                if (l.startsWith('Compiling ')) { return false; }
                if (l.startsWith("For more information, try '--help'")) { return false; }
                return true;
            })
            .map(l => {
                try {
                    const parsed = JSON.parse(l);
                    if (parsed?.error?.message) {
                        return parsed.error.message;
                    }
                    if (parsed?.warning?.message) {
                        return `Warning: ${parsed.warning.message}`;
                    }
                } catch {
                    // not JSON, keep as-is
                }
                return l;
            });

        return lines.length > 0 ? lines.join('\n') : stderr;
    }
}
