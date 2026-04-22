import * as vscode from 'vscode';
import * as rqClient from '../rqClient';
import { RequestExplorerProvider, RequestTreeItem } from '../requestExplorer';
import { performOAuth2Flow } from '../auth';
import { getWebviewContent, getErrorWebviewContent } from '../ui/webviewGenerator';

interface RequestExecutionContext {
    requestName: string;
    sourceDirectory: string | undefined;
    environment: string | undefined;
    variables: Record<string, string> | undefined;
}

export class RequestRunner {
    private resultsPanel: vscode.WebviewPanel | undefined;
    private lastExecutionContext: RequestExecutionContext | undefined;
    private lastBody: string | undefined;

    constructor(
        private context: vscode.ExtensionContext,
        private outputChannel: vscode.OutputChannel
    ) {}

    public registerCommands(provider: RequestExplorerProvider) {
        this.context.subscriptions.push(
            vscode.commands.registerCommand('rq.runRequest', (item) => this.runRequest(item, provider)),
            vscode.commands.registerCommand('rq.runRequestWithVariables', (item) => this.runRequestWithVariables(item, provider))
        );
    }

    public async runRequest(requestItem: RequestTreeItem, provider: RequestExplorerProvider) {
        if (!requestItem.request) {
            vscode.window.showErrorMessage('Cannot run: No request information available');
            return;
        }

        provider.setItemLoading(requestItem, true);
        try {
            const requestName = requestItem.request.name;
            const sourceDirectory = this.getSourceDirectory();
            const environment = provider.getSelectedEnvironment();

            this.outputChannel.appendLine(`Selected environment: ${environment || '(none)'}`);

            const variables = await this.handleOAuth2(requestName, sourceDirectory, environment) || {};
            await this.collectRequiredVariables(requestName, sourceDirectory, environment, variables);

            await this.executeRequestLogic(requestName, sourceDirectory, environment, Object.keys(variables).length > 0 ? variables : undefined);

        } catch (error) {
            await this.handleError(error);
        } finally {
            provider.setItemLoading(requestItem, false);
        }
    }

    public async runRequestWithVariables(requestItem: RequestTreeItem, provider: RequestExplorerProvider) {
        if (!requestItem.request) {
            vscode.window.showErrorMessage('Cannot run: No request information available');
            return;
        }

        provider.setItemLoading(requestItem, true);
        try {
            const requestName = requestItem.request.name;
            const sourceDirectory = this.getSourceDirectory();
            const environment = provider.getSelectedEnvironment();

            const variables = await this.handleOAuth2(requestName, sourceDirectory, environment) || {};
            await this.collectUserVariables(variables);
            await this.collectRequiredVariables(requestName, sourceDirectory, environment, variables);

            await this.executeRequestLogic(requestName, sourceDirectory, environment, Object.keys(variables).length > 0 ? variables : undefined);

        } catch (error) {
            await this.handleError(error);
        } finally {
            provider.setItemLoading(requestItem, false);
        }
    }

    private getSourceDirectory(): string | undefined {
        return vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    }

    private async handleOAuth2(
        requestName: string,
        sourceDirectory: string | undefined,
        environment: string | undefined
    ): Promise<Record<string, string> | undefined> {
        let requestDetails: rqClient.RequestShowOutput;
        try {
            requestDetails = await rqClient.showRequest(requestName, sourceDirectory, environment, true, true);
        } catch (error) {
            const errorMessage = error instanceof Error ? error.message : String(error);
            this.outputChannel.appendLine(`Warning: Failed to check/apply auth for request: ${errorMessage}`);
            return undefined;
        }

        this.outputChannel.appendLine(`Auth for '${requestName}': ${JSON.stringify(requestDetails.auth ?? null)}`);

        if (requestDetails.auth && (requestDetails.auth.type === 'oauth2_authorization_code' || requestDetails.auth.type === 'oauth2_implicit')) {
            this.outputChannel.appendLine(`Detected OAuth2 auth: ${requestDetails.auth.name} (${requestDetails.auth.type})`);

            const authConfig = await rqClient.showAuthConfig(
                requestDetails.auth.name,
                sourceDirectory,
                environment
            );

            this.outputChannel.appendLine(`Performing OAuth2 flow...`);
            const accessToken = await performOAuth2Flow(authConfig, this.context, this.outputChannel);
            this.outputChannel.appendLine(`OAuth2 token obtained, injecting as auth_token variable`);

            return { auth_token: accessToken };
        }
        return undefined;
    }

    private async collectRequiredVariables(
        requestName: string,
        sourceDirectory: string | undefined,
        environment: string | undefined,
        variables: Record<string, string>
    ) {
        let requestDetails: rqClient.RequestShowOutput;
        try {
            requestDetails = await rqClient.showRequest(requestName, sourceDirectory, environment, false);
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
                throw new Error('Cancelled by user');
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
                throw new Error('Cancelled by user');
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

    private async executeRequestLogic(
        requestName: string,
        sourceDirectory: string | undefined,
        environment: string | undefined,
        variables: Record<string, string> | undefined
    ) {
        this.lastExecutionContext = { requestName, sourceDirectory, environment, variables };

        await vscode.window.withProgress({
            location: vscode.ProgressLocation.Notification,
            title: `Running request: ${requestName}`,
            cancellable: false
        }, async () => {
            const result = await rqClient.executeRequest({ requestName, sourceDirectory, environment, variables });

            if (result.results.length > 0) {
                await this.handleSuccessfulExecution(requestName, variables, result);
            } else {
                await this.handleFailedExecution(requestName, variables, result);
            }
        });
    }

    private async handleSuccessfulExecution(
        requestName: string,
        variables: Record<string, string> | undefined,
        result: rqClient.ExecuteRequestResult
    ) {
        // Log the successful response to the output channel
        if (result.results.length > 0) {
            const responseData = result.results[0];
            const logContent = this.formatResponseForLog(responseData);
            this.logOutput(`Request: ${requestName}`, logContent, variables);
        }

        if (result.stderr) {
            this.outputChannel.appendLine(`STDERR output:\n${result.stderr}`);
        }

        if (this.resultsPanel) {
            this.resultsPanel.title = `RQ: ${requestName}`;
            this.resultsPanel.reveal(vscode.ViewColumn.One);
        } else {
            this.createWebviewPanel(requestName);
        }

        this.lastBody = result.results[0].body;
        const html = await getWebviewContent(this.context, result.results[0]);
        if (this.resultsPanel) {
            this.resultsPanel.webview.html = html;
        }
    }

    private async handleFailedExecution(
        requestName: string,
        variables: Record<string, string> | undefined,
        result: rqClient.ExecuteRequestResult
    ) {
        const fullError = result.stderr
            ? this.cleanErrorMessage(result.stderr)
            : `No results returned for request: ${requestName}`;

        this.logOutput(`Request Failed: ${requestName}`, fullError, variables);
        await this.showErrorPanel(requestName, fullError);
    }

    private createWebviewPanel(requestName: string) {
        this.resultsPanel = vscode.window.createWebviewPanel(
            'rqRequestResult',
            `RQ: ${requestName}`,
            vscode.ViewColumn.One,
            { enableScripts: true, retainContextWhenHidden: true }
        );

        this.resultsPanel.onDidDispose(() => {
            this.resultsPanel = undefined;
        });

        this.resultsPanel.webview.onDidReceiveMessage(message => {
            if (message.command === 'runAgain' && this.lastExecutionContext) {
                this.executeRequestLogic(
                    this.lastExecutionContext.requestName,
                    this.lastExecutionContext.sourceDirectory,
                    this.lastExecutionContext.environment,
                    this.lastExecutionContext.variables
                ).catch(error => this.handleError(error));
            }
            if (message.command === 'copyBody' && this.lastBody !== undefined) {
                vscode.env.clipboard.writeText(this.lastBody);
                vscode.window.showInformationMessage('Body copied to clipboard');
            }
        }, undefined, this.context.subscriptions);
    }

    private formatResponseForLog(result: rqClient.RequestExecutionResult): string {
        let output = `Status: ${result.status}\n`;
        output += `Time: ${result.elapsed_ms}ms\n`;
        output += `Method: ${result.method}\n`;
        output += `URL: ${result.url}\n\n`;
        
        output += `Response Headers:\n`;
        for (const [key, value] of Object.entries(result.response_headers)) {
            output += `${key}: ${value}\n`;
        }
        
        output += `\nBody:\n`;
        try {
            const bodyJson = JSON.parse(result.body);
            output += JSON.stringify(bodyJson, null, 2);
        } catch (e) {
            output += result.body;
        }
        output += `\n`;
        return output;
    }

    private logOutput(title: string, content: string, variables?: Record<string, string>) {
        this.outputChannel.appendLine(`\n${'='.repeat(80)}`);
        this.outputChannel.appendLine(title);
        if (variables) {
            this.outputChannel.appendLine(`Variables: ${JSON.stringify(variables)}`);
        }
        this.outputChannel.appendLine(`Time: ${new Date().toISOString()}`);
        this.outputChannel.appendLine(`${'='.repeat(80)}`);
        this.outputChannel.appendLine(content);
        this.outputChannel.appendLine(`${'='.repeat(80)}\n`);
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
        if (errorMessage === 'Cancelled by user') {
            return;
        }
        this.outputChannel.appendLine(`\nERROR: ${errorMessage}\n`);
        const requestName = this.lastExecutionContext?.requestName ?? 'Request';
        await this.showErrorPanel(requestName, errorMessage);
    }

    private async showErrorPanel(requestName: string, errorMessage: string) {
        if (this.resultsPanel) {
            this.resultsPanel.title = `RQ: ${requestName}`;
            this.resultsPanel.reveal(vscode.ViewColumn.One);
        } else {
            this.createWebviewPanel(requestName);
        }

        let requestDetails: rqClient.RequestShowOutput | undefined;
        try {
            requestDetails = await rqClient.showRequest(
                requestName,
                this.lastExecutionContext?.sourceDirectory,
                this.lastExecutionContext?.environment,
                true
            );
        } catch {
            // best-effort: show error panel without request details
        }

        if (this.resultsPanel) {
            this.resultsPanel.webview.html = await getErrorWebviewContent(this.context, requestName, errorMessage, requestDetails);
        }
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