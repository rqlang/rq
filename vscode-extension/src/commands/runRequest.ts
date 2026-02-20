import * as vscode from 'vscode';
import * as cliService from '../cliService';
import { RequestExplorerProvider, RequestTreeItem } from '../requestExplorer';
import { performOAuth2Flow } from '../auth';
import { getWebviewContent } from '../ui/webviewGenerator';

interface RequestExecutionContext {
    requestName: string;
    sourceDirectory: string | undefined;
    environment: string | undefined;
    variables: Record<string, string> | undefined;
}

export class RequestRunner {
    private resultsPanel: vscode.WebviewPanel | undefined;
    private lastExecutionContext: RequestExecutionContext | undefined;

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
        if (cliService.isCliInstalling()) {
            vscode.window.showWarningMessage('rq CLI is being installed. Please wait until installation completes.');
            return;
        }

        if (!cliService.isCliBinaryAvailable()) {
            await cliService.handleCliNotFoundError();
            return;
        }

        try {
            if (!requestItem.request) {
                vscode.window.showErrorMessage('Cannot run: No request information available');
                return;
            }

            const requestName = requestItem.request.name;
            const sourceDirectory = this.getSourceDirectory();
            const environment = provider.getSelectedEnvironment();

            this.outputChannel.appendLine(`Selected environment: ${environment || '(none)'}`);

            const variables = await this.handleOAuth2(requestName, sourceDirectory, environment);
            await this.executeRequestLogic(requestName, sourceDirectory, environment, variables);

        } catch (error) {
            this.handleError(error);
        }
    }

    public async runRequestWithVariables(requestItem: RequestTreeItem, provider: RequestExplorerProvider) {
        if (cliService.isCliInstalling()) {
            vscode.window.showWarningMessage('rq CLI is being installed. Please wait until installation completes.');
            return;
        }

        if (!cliService.isCliBinaryAvailable()) {
            await cliService.handleCliNotFoundError();
            return;
        }

        try {
            if (!requestItem.request) {
                vscode.window.showErrorMessage('Cannot run: No request information available');
                return;
            }

            const requestName = requestItem.request.name;
            const sourceDirectory = this.getSourceDirectory();
            const environment = provider.getSelectedEnvironment();

            const variables = await this.handleOAuth2(requestName, sourceDirectory, environment) || {};
            await this.collectUserVariables(variables);

            await this.executeRequestLogic(requestName, sourceDirectory, environment, Object.keys(variables).length > 0 ? variables : undefined);

        } catch (error) {
            this.handleError(error);
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
        try {
            const requestDetails = await cliService.showRequest(requestName, sourceDirectory, environment);
            this.outputChannel.appendLine(`Request details for '${requestName}': ${JSON.stringify(requestDetails)}`);

            if (requestDetails.auth && (requestDetails.auth.type === 'oauth2_authorization_code' || requestDetails.auth.type === 'oauth2_implicit')) {
                this.outputChannel.appendLine(`Detected OAuth2 auth: ${requestDetails.auth.name} (${requestDetails.auth.type})`);

                const authConfig = await cliService.showAuthConfig(
                    requestDetails.auth.name,
                    sourceDirectory,
                    environment
                );

                this.outputChannel.appendLine(`Performing OAuth2 flow...`);
                const accessToken = await performOAuth2Flow(authConfig, this.context, this.outputChannel);
                this.outputChannel.appendLine(`OAuth2 token obtained, injecting as auth_token variable`);

                return { auth_token: accessToken };
            }
        } catch (error) {
            const errorMessage = error instanceof Error ? error.message : String(error);
            this.outputChannel.appendLine(`Warning: Failed to check/apply auth for request: ${errorMessage}`);
            throw error;
        }
        return undefined;
    }

    private async collectUserVariables(variables: Record<string, string>) {
        let addingVariables = true;
        while (addingVariables) {
            const variableInput = await vscode.window.showInputBox({
                prompt: 'Enter variable in format: variable=value (or leave empty to execute)',
                placeHolder: 'e.g., color=red or userId=123',
                validateInput: this.validateVariableInput
            });

            if (variableInput === undefined) {
                throw new Error('Cancelled by user');
            }

            if (!variableInput) {
                addingVariables = false;
            } else {
                const [varName, varValue] = variableInput.split('=').map(s => s.trim());
                variables[varName] = varValue;

                const addMore = await vscode.window.showQuickPick(['Add another variable', 'Execute request'], {
                    placeHolder: `Added: ${varName}=${varValue}. What would you like to do?`
                });

                if (addMore === undefined || addMore === 'Execute request') {
                    addingVariables = false;
                }
            }
        }
    }

    private validateVariableInput(value: string): string | null {
        if (!value) { return null; }
        if (!value.includes('=')) { return 'Variable must be in format: variable=value'; }
        const parts = value.split('=');
        if (parts.length !== 2 || !parts[0].trim() || !parts[1].trim()) { return 'Invalid format. Use: variable=value'; }
        if (parts[0].includes('"') || parts[1].includes('"')) { return 'Double quotes (") are not allowed in variable names or values'; }
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
            const result = await cliService.executeRequest({ requestName, sourceDirectory, environment, variables });

            if (result.results.length > 0) {
                await this.handleSuccessfulExecution(requestName, variables, result);
            } else {
                this.handleFailedExecution(requestName, variables, result);
            }
        });
    }

    private async handleSuccessfulExecution(
        requestName: string,
        variables: Record<string, string> | undefined,
        result: cliService.ExecuteRequestResult
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

        const html = await getWebviewContent(this.context, result.results[0]);
        if (this.resultsPanel) {
            this.resultsPanel.webview.html = html;
        }
    }

    private handleFailedExecution(
        requestName: string,
        variables: Record<string, string> | undefined,
        result: cliService.ExecuteRequestResult
    ) {
        let errorMsg = result.stderr || `No results returned for request: ${requestName}`;
        if (result.stderr) {
            errorMsg = this.cleanErrorMessage(result.stderr);
        }

        vscode.window.showErrorMessage(errorMsg);

        if (result.stderr) {
            this.logOutput(`Request Failed: ${requestName}`, result.stderr, variables);
        }
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
                );
            }
        }, undefined, this.context.subscriptions);
    }

    private formatResponseForLog(result: cliService.RequestExecutionResult): string {
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

    private handleError(error: unknown) {
        const errorMessage = error instanceof Error ? error.message : 'Unknown error';
        if (errorMessage !== 'Cancelled by user') {
            vscode.window.showErrorMessage(`Failed to run request: ${errorMessage}`);
            this.outputChannel.appendLine(`\nERROR: ${errorMessage}\n`);
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
            });

        return lines.length > 0 ? lines.join('\n') : stderr;
    }
}