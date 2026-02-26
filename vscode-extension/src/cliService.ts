import * as vscode from 'vscode';
import { spawn, SpawnOptionsWithoutStdio } from 'child_process';
import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import { normalizePath } from './utils';

interface ExecError extends Error {
    stdout?: string;
    stderr?: string;
    code?: number;
}

function spawnAsync(command: string, args: string[], options: SpawnOptionsWithoutStdio = {}): Promise<{ stdout: string; stderr: string }> {
    return new Promise((resolve, reject) => {
        const proc = spawn(command, args, options);
        let stdout = '';
        let stderr = '';

        if (proc.stdout) {
            proc.stdout.on('data', data => stdout += data);
        }
        if (proc.stderr) {
            proc.stderr.on('data', data => stderr += data);
        }

        proc.on('error', error => reject(error));
        proc.on('close', code => {
            if (code === 0) {
                resolve({ stdout, stderr });
            } else {
                const error = new Error(`Command failed with code ${code}`) as ExecError;
                error.code = code ?? undefined;
                error.stdout = stdout;
                error.stderr = stderr;
                reject(error);
            }
        });
    });
}

function getErrorMessage(error: unknown): string {
    if (error instanceof Error && 'stderr' in error) {
        const execError = error as ExecError;
        if (execError.stderr) {
            const lines = execError.stderr.split('\n')
                .map(l => l.trim())
                .filter(l => {
                    if (l.length === 0) { return false; }
                    if (l.startsWith('Finished ')) { return false; }
                    if (l.startsWith('Running `')) { return false; }
                    if (l.startsWith('Compiling ')) { return false; }
                    return true;
                });
            return lines.length > 0 ? lines.join('\n') : execError.stderr;
        }
    }
    return error instanceof Error ? error.message : 'Unknown error';
}

let outputChannel: vscode.OutputChannel | undefined;
let diagnosticCollection: vscode.DiagnosticCollection | undefined;
let cliInstalling = false;
let installFinishedCallback: (() => void) | undefined;

export function onInstallFinished(callback: () => void): void {
    installFinishedCallback = callback;
}

export function setCliInstalling(installing: boolean): void {
    cliInstalling = installing;
}

export function isCliInstalling(): boolean {
    return cliInstalling;
}

export function isCliNotFound(error: unknown): boolean {
    if (error instanceof Error) {
        return error.message.includes('ENOENT') || (error as NodeJS.ErrnoException).code === 'ENOENT';
    }
    return false;
}

export async function handleCliNotFoundError(): Promise<void> {
    const action = await vscode.window.showWarningMessage(
        'rq CLI is not installed.',
        'Install Now'
    );
    if (action === 'Install Now') {
        await promptInstallCli();
    }
}

export function isCliBinaryAvailable(): boolean {
    if (isDevelopment()) {
        return true;
    }

    const customPath = vscode.workspace.getConfiguration('rq').get<string>('cli.path', '');
    if (customPath) {
        return fs.existsSync(customPath);
    }

    const resolved = resolveRqBinary();
    return resolved !== 'rq';
}

export function setOutputChannel(channel: vscode.OutputChannel): void {
    outputChannel = channel;
}

export function setDiagnosticCollection(collection: vscode.DiagnosticCollection): void {
    diagnosticCollection = collection;
}

function parseAndReportErrors(stderr: string, cwd?: string): void {
    if (!diagnosticCollection) {
        return;
    }

    diagnosticCollection.clear();

    const errorRegex = /Syntax error in (.+) at line (\d+), column (\d+): (.+)/g;
    let match;
    
    const diagnosticsMap = new Map<string, vscode.Diagnostic[]>();

    while ((match = errorRegex.exec(stderr)) !== null) {
        const rawFilePath = match[1].trim();
        const line = parseInt(match[2], 10) - 1; 
        const column = parseInt(match[3], 10) - 1;
        const message = match[4];

        if (outputChannel) {
            outputChannel.appendLine(`[Error Parsing] Found error in: ${rawFilePath} (cwd: ${cwd})`);
        }
        console.log(`[Error Parsing] Found error in: ${rawFilePath} (cwd: ${cwd})`);

        let filePath = rawFilePath;
        const looksLikeAbsolute = rawFilePath.startsWith('/') || /^[a-zA-Z]:/.test(rawFilePath) || rawFilePath.startsWith('\\');

        if (!path.isAbsolute(rawFilePath) && !looksLikeAbsolute) {
            const basePath = cwd || vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
             if (outputChannel) {
                 outputChannel.appendLine(`[Error Parsing] Resolving relative path with base: ${basePath}`);
             }
             console.log(`[Error Parsing] Resolving relative path with base: ${basePath}`);
            if (basePath) {
                filePath = path.resolve(basePath, rawFilePath);
            }
        } else if (looksLikeAbsolute && !path.isAbsolute(rawFilePath)) {
             filePath = rawFilePath;
        }
        
        filePath = normalizePath(filePath);
        if (outputChannel) {
            outputChannel.appendLine(`[Error Parsing] Normalized path: ${filePath}`);
        }
        console.log(`[Error Parsing] Normalized path: ${filePath}`);

        const range = new vscode.Range(line, column, line, Number.MAX_VALUE);
        const diagnostic = new vscode.Diagnostic(range, message, vscode.DiagnosticSeverity.Error);
        diagnostic.source = 'rq-cli';

        const uri = vscode.Uri.file(filePath);
        const uriStr = uri.toString();
        
        if (!diagnosticsMap.has(uriStr)) {
            diagnosticsMap.set(uriStr, []);
        }
        diagnosticsMap.get(uriStr)?.push(diagnostic);
    }

    for (const [uriStr, diagnostics] of diagnosticsMap) {
        diagnosticCollection.set(vscode.Uri.parse(uriStr), diagnostics);
    }
}

function logCliExecution(command: string, cwd?: string): void {
    if (outputChannel) {
        const timestamp = new Date().toISOString();
        outputChannel.appendLine(`[${timestamp}] Executing: ${command}`);
        if (cwd) {
            outputChannel.appendLine(`[${timestamp}] Working directory: ${cwd}`);
        }
    }
}

function logCliError(operation: string, error: unknown): void {
    if (outputChannel) {
        const timestamp = new Date().toISOString();
        outputChannel.appendLine(`[${timestamp}] ERROR: ${operation}`);
        
        if (error instanceof Error && 'stderr' in error) {
            const execError = error as { stdout?: string; stderr?: string; code?: number };
            if (execError.stderr) {
                outputChannel.appendLine(`[${timestamp}] stderr: ${execError.stderr}`);
            }
            if (execError.stdout) {
                outputChannel.appendLine(`[${timestamp}] stdout: ${execError.stdout}`);
            }
            if (execError.code !== undefined) {
                outputChannel.appendLine(`[${timestamp}] Exit code: ${execError.code}`);
            }
        }
        
        outputChannel.appendLine(`[${timestamp}] Error message: ${error instanceof Error ? error.message : 'Unknown error'}`);
        outputChannel.appendLine('');
    }
}

export interface Environment {
    name: string;
}

export type EnvironmentListOutput = string[];

export interface AuthConfig {
    name: string;
}

export interface AuthListEntry {
    name: string;
    auth_type: string;
}

export type AuthListOutput = AuthListEntry[];

interface AuthShowRaw {
    'Auth Configuration': string;
    Type: string;
    Fields: Record<string, string>;
    Environment?: string;
}

export interface AuthShowOutput {
    name: string;
    auth_type: string;
    fields: Record<string, string>;
    environment?: string;
}

export interface RequestInfo {
    name: string;
    endpoint: string | null;
    file: string;
}

export type RequestListOutput = RequestInfo[];

export interface ListRequestsResult {
    requests: RequestInfo[];
    errors?: string[];
}

interface RequestShowRaw {
    Request: string;
    URL: string;
    Method: string;
    Headers: Record<string, string>;
    Body?: string;
    Auth?: {
        name: string;
        type: string;
    };
}

export interface RequestShowOutput {
    name: string;
    auth?: {
        name: string;
        type: string;
    };
}

let extensionMode: vscode.ExtensionMode | undefined;

export function setExtensionMode(mode: vscode.ExtensionMode): void {
    extensionMode = mode;
}

function isDevelopment(): boolean {
    return extensionMode === vscode.ExtensionMode.Development || 
           vscode.workspace.getConfiguration('rq').get<boolean>('useDevelopmentCli', false);
}

let cachedCliPath: string | null = null;
let cachedExtensionPath: string | null = null;

function findCliPath(): string | null {
    if (cachedCliPath !== null) {
        return cachedCliPath;
    }

    const workspaceFolders = vscode.workspace.workspaceFolders;
    if (workspaceFolders) {
        for (const folder of workspaceFolders) {
            const cliPath = path.join(folder.uri.fsPath, 'cli');
            const cargoTomlPath = path.join(cliPath, 'Cargo.toml');
            
            try {
                if (fs.existsSync(cargoTomlPath)) {
                    console.log(`Found CLI at: ${cliPath}`);
                    cachedCliPath = cliPath;
                    return cliPath;
                }
            } catch (_) { /* empty */ }
        }
    }

    cachedCliPath = null;
    return null;
}

export function setExtensionPath(extensionPath: string): void {
    cachedExtensionPath = extensionPath;

    const repoRoot = path.dirname(extensionPath);
    const cliPath = path.join(repoRoot, 'cli');
    const cargoTomlPath = path.join(cliPath, 'Cargo.toml');
    
    try {
        if (fs.existsSync(cargoTomlPath)) {
            console.log(`Extension path based CLI found at: ${cliPath}`);
            cachedCliPath = cliPath;
        }
    } catch (error) {
        console.warn('Could not find CLI using extension path:', error);
    }
}

function getCliCommand(): { executable: string; args: string[]; cwd?: string } {
    if (isDevelopment()) {        
        return { executable: 'cargo', args: ['run', '--'], cwd: findCliPath() || undefined };
    }

    const workspaceCwd = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;

    const customPath = vscode.workspace.getConfiguration('rq').get<string>('cli.path', '');
    if (customPath) {
        try {
            if (fs.existsSync(customPath)) {
                return { executable: customPath, args: [], cwd: workspaceCwd };
            }
        } catch (_) { /* empty */ }
    }

    return { executable: resolveRqBinary(), args: [], cwd: workspaceCwd };
}

export async function checkCliVersion(extensionId: string): Promise<void> {
    if (isDevelopment()) {
        return;
    }

    const extension = vscode.extensions.getExtension(extensionId);
    if (!extension) {
        return;
    }
    const extensionVersion: string = extension.packageJSON.version;
    const isDevVersion = extensionVersion.includes('-dev.');

    const { executable, args: baseArgs, cwd } = getCliCommand();
    const args = [...baseArgs, '--version'];

    let cliVersion: string | null = null;
    try {
        const { stdout } = await spawnAsync(executable, args, { cwd });
        const match = stdout.trim().match(/(\d+\.\d+\.\d+(?:-dev\.\d+)?)/);
        if (match) {
            cliVersion = match[1];
        }
    } catch (_) { /* empty */ }

    if (cliVersion === null) {
        setCliInstalling(true);
        const selection = await vscode.window.showWarningMessage(
            `rq CLI is not installed. The extension requires CLI version ${extensionVersion}.`,
            'Install Now'
        );
        if (selection === 'Install Now') {
            await startInstall(extensionVersion, isDevVersion);
        } else {
            setCliInstalling(false);
        }
        return;
    }

    if (cliVersion !== extensionVersion) {
        setCliInstalling(true);
        const selection = await vscode.window.showWarningMessage(
            `rq CLI version mismatch: installed ${cliVersion}, expected ${extensionVersion}.`,
            'Update Now'
        );
        if (selection === 'Update Now') {
            await runInstallScript(extensionVersion, isDevVersion);
        } else {
            setCliInstalling(false);
        }
    }
}

export async function promptInstallCli(): Promise<void> {
    const extension = vscode.extensions.getExtension('rq-lang.rq-language');
    if (!extension) {
        return;
    }
    const extensionVersion: string = extension.packageJSON.version;
    const isDevVersion = extensionVersion.includes('-dev.');

    const selection = await vscode.window.showWarningMessage(
        'rq CLI is not installed.',
        'Install Now'
    );
    if (selection === 'Install Now') {
        setCliInstalling(true);
        await startInstall(extensionVersion, isDevVersion);
    }
}

async function startInstall(extensionVersion: string, isDevVersion: boolean): Promise<void> {
    const installDir = getLocalInstallDir();
    const config = vscode.workspace.getConfiguration('rq');
    
    const binaryName = process.platform === 'win32' ? 'rq.exe' : 'rq';
    const fullPath = path.join(installDir, binaryName);
    await config.update('cli.path', fullPath, vscode.ConfigurationTarget.Global);

    await runInstallScript(extensionVersion, isDevVersion);
}

function getLocalInstallDir(): string {
    if (process.platform === 'win32') {
        return path.join(process.env.LOCALAPPDATA || path.join(os.homedir(), 'AppData', 'Local'), 'rq');
    }
    return path.join(os.homedir(), '.local', 'bin');
}

function getPathInstallDir(): string {
    if (process.platform === 'win32') {
        return path.join(process.env.LOCALAPPDATA || path.join(os.homedir(), 'AppData', 'Local'), 'rq');
    }
    return '/usr/local/bin';
}

function resolveRqBinary(): string {
    const binaryName = process.platform === 'win32' ? 'rq.exe' : 'rq';
    const knownDirs = [
        getLocalInstallDir(),
        getPathInstallDir()
    ];

    for (const dir of knownDirs) {
        const fullPath = path.join(dir, binaryName);
        try {
            if (fs.existsSync(fullPath)) {
                return fullPath;
            }
        } catch (_) { /* empty */ }
    }

    return 'rq';
}

async function runInstallScript(releaseTag: string, isDev: boolean = false): Promise<void> {
    if (!cachedExtensionPath) {
        vscode.window.showErrorMessage('Cannot locate extension path. Please reinstall the extension.');
        return;
    }

    const scriptsDir = path.join(cachedExtensionPath, 'scripts');
    let command: string;
    let taskName = 'Install rq CLI';

    if (isDev) {
        const devScriptPath = path.join(scriptsDir, 'install-rq-dev.sh');
        const fs = await import('fs');
        if (fs.existsSync(devScriptPath)) {
            const localDir = getLocalInstallDir();
            
            if (process.platform === 'win32') {
                const devScriptPs1 = path.join(scriptsDir, 'install-rq-dev.ps1');
                command = `powershell -ExecutionPolicy Bypass -File "${devScriptPs1}" -InstallDir "${localDir}"`;
            } else {
                command = `bash "${devScriptPath}" --install-dir "${localDir}"`;
            }

            taskName = 'Install rq CLI (Dev)';

            await runInstallationTask(command, taskName);
            return;
        }
    }

    const localDir = getLocalInstallDir();

    if (process.platform === 'win32') {
        const scriptPath = path.join(scriptsDir, 'install-rq.ps1');
        command = `powershell -ExecutionPolicy Bypass -File "${scriptPath}" -ReleaseTag "${releaseTag}" -InstallDir "${localDir}"`;
    } else {
        const scriptPath = path.join(scriptsDir, 'install-rq.sh');
        command = `bash "${scriptPath}" --release-tag "${releaseTag}" --install-dir "${localDir}"`;
    }

    await runInstallationTask(command, taskName);
}

async function runInstallationTask(command: string, name: string): Promise<void> {
    const task = new vscode.Task(
        { type: 'shell', task: name },
        vscode.TaskScope.Workspace,
        name,
        'rq',
        new vscode.ShellExecution(command)
    );

    const execution = await vscode.tasks.executeTask(task);
    
    const disposable = vscode.tasks.onDidEndTaskProcess(e => {
        if (e.execution === execution) {
            disposable.dispose();
            setCliInstalling(false);
            if (e.exitCode === 0) {
                installFinishedCallback?.();
                vscode.window.showInformationMessage('rq CLI installation completed successfully.');
            } else {
                vscode.window.showErrorMessage(`rq CLI installation failed with exit code ${e.exitCode}`);
            }
        }
    });
}

export async function listEnvironments(sourceDirectory?: string): Promise<string[]> {
    try {
        const { executable, args: baseArgs, cwd } = getCliCommand();
        const args = [...baseArgs, 'env', 'list'];
        if (sourceDirectory) {
            args.push('-s', sourceDirectory);
        }
        args.push('-o', 'json');
        
        const fullCommand = `${executable} ${args.join(' ')}`;
        logCliExecution(fullCommand, cwd);
        console.log(`Executing: ${fullCommand}`);
        if (cwd) {
            console.log(`Working directory: ${cwd}`);
        }
        
        const { stdout, stderr } = await spawnAsync(executable, args, { cwd });
        
        if (stderr) {
            console.error('CLI stderr:', stderr);
            parseAndReportErrors(stderr, cwd);
        } else {
            if (diagnosticCollection) {
                diagnosticCollection.clear();
            }
        }
        
        const output: EnvironmentListOutput = JSON.parse(stdout);
        return output;
    } catch (error) {
        console.error('Failed to list environments:', error);
        logCliError('Failed to list environments', error);
        
        if (error instanceof Error && 'stderr' in error) {
            parseAndReportErrors((error as ExecError).stderr || '', getCliCommand().cwd);
        }

        throw new Error(`Failed to list environments: ${getErrorMessage(error)}`);
    }
}

export async function listAuthConfigs(sourceDirectory?: string): Promise<AuthListEntry[]> {
    try {
        const { executable, args: baseArgs, cwd } = getCliCommand();
        const args = [...baseArgs, 'auth', 'list'];
        if (sourceDirectory) {
            args.push('-s', sourceDirectory);
        }
        args.push('-o', 'json');
        
        const fullCommand = `${executable} ${args.join(' ')}`;
        
        logCliExecution(fullCommand, cwd);
        console.log(`Executing: ${fullCommand}`);
        if (cwd) {
            console.log(`Working directory: ${cwd}`);
        }
        
        const { stdout, stderr } = await spawnAsync(executable, args, { cwd });
        
        if (stderr) {
            console.error('CLI stderr:', stderr);
            parseAndReportErrors(stderr, cwd);
        } else {
            if (diagnosticCollection) {
                diagnosticCollection.clear();
            }
        }
        
        const output: AuthListOutput = JSON.parse(stdout);
        return output;
    } catch (error) {
        console.error('Failed to list auth configs:', error);
        logCliError('Failed to list auth configs', error);

        if (error instanceof Error && 'stderr' in error) {
            parseAndReportErrors((error as ExecError).stderr || '', getCliCommand().cwd);
        }

        throw new Error(`Failed to list auth configs: ${getErrorMessage(error)}`);
    }
}

export async function showAuthConfig(
    name: string, 
    sourceDirectory?: string, 
    environment?: string
): Promise<AuthShowOutput> {
    try {
        const { executable, args: baseArgs, cwd } = getCliCommand();
        const args = [...baseArgs, 'auth', 'show', '-n', name];
        if (sourceDirectory) {
            args.push('-s', sourceDirectory);
        }
        if (environment) {
            args.push('-e', environment);
        }
        args.push('-o', 'json');
        
        const fullCommand = `${executable} ${args.join(' ')}`;
        
        logCliExecution(fullCommand, cwd);
        console.log(`Executing: ${fullCommand}`);
        if (cwd) {
            console.log(`Working directory: ${cwd}`);
        }
        
        const { stdout, stderr } = await spawnAsync(executable, args, { cwd });
        
        if (stderr) {
            console.error('CLI stderr:', stderr);
            parseAndReportErrors(stderr, cwd);
        } else {
            if (diagnosticCollection) {
                diagnosticCollection.clear();
            }
        }
        
        const raw: AuthShowRaw = JSON.parse(stdout);
        const output: AuthShowOutput = {
            name: raw['Auth Configuration'],
            auth_type: raw.Type,
            fields: raw.Fields,
            environment: raw.Environment,
        };
        return output;
    } catch (error) {
        console.error('Failed to show auth config:', error);
        logCliError('Failed to show auth config', error);

        if (error instanceof Error && 'stderr' in error) {
            parseAndReportErrors((error as ExecError).stderr || '', getCliCommand().cwd);
        }

        throw new Error(`Failed to show auth config: ${getErrorMessage(error)}`);
    }
}

export async function listRequests(sourceDirectory?: string): Promise<ListRequestsResult> {
    try {
        const { executable, args: baseArgs, cwd } = getCliCommand();
        const args = [...baseArgs, 'request', 'list'];
        if (sourceDirectory) {
            args.push('-s', sourceDirectory);
        }
        args.push('-o', 'json');
        
        const fullCommand = `${executable} ${args.join(' ')}`;
        
        logCliExecution(fullCommand, cwd);
        console.log(`Executing: ${fullCommand}`);
        if (cwd) {
            console.log(`Working directory: ${cwd}`);
        }
        
        const { stdout, stderr } = await spawnAsync(executable, args, { cwd });
        
        let errors: string[] = [];
        if (stderr) {
            console.warn('CLI warnings/errors:');
            console.warn(stderr);
            parseAndReportErrors(stderr, cwd);
            if (stderr.includes('Warning: Failed to parse')) {
                errors = stderr.split('\n')
                    .filter(line => line.includes('Warning: Failed to parse'))
                    .map(line => line.trim());
                
                if (errors.length > 0) {
                    console.warn(`Found ${errors.length} file(s) with parse errors - check Output panel for details`);
                }
            }
        } else {
            if (diagnosticCollection) {
                diagnosticCollection.clear();
            }
        }
        
        const output: RequestListOutput = JSON.parse(stdout);
        
        output.forEach(item => {
            item.file = normalizePath(item.file);
        });

        return {
            requests: output,
            errors: errors.length > 0 ? errors : undefined
        };
    } catch (error) {
        console.error('Failed to list requests:', error);
        logCliError('Failed to list requests', error);
        
        if (error instanceof Error && 'stderr' in error) {
            parseAndReportErrors((error as ExecError).stderr || '', getCliCommand().cwd);
        }

        throw new Error(`Failed to list requests: ${getErrorMessage(error)}`);
    }
}

export async function showRequest(
    requestName: string,
    sourceDirectory?: string,
    environment?: string
): Promise<RequestShowOutput> {
    try {
        const { executable, args: baseArgs, cwd } = getCliCommand();
        const args = [...baseArgs, 'request', 'show', '-n', requestName];
        if (sourceDirectory) {
            args.push('-s', sourceDirectory);
        }
        if (environment) {
            args.push('-e', environment);
        }
        args.push('-o', 'json');
        
        const fullCommand = `${executable} ${args.join(' ')}`;
        
        logCliExecution(fullCommand, cwd);
        console.log(`Executing: ${fullCommand}`);
        if (cwd) {
            console.log(`Working directory: ${cwd}`);
        }
        
        const { stdout, stderr } = await spawnAsync(executable, args, { cwd });
        
        if (stderr) {
            console.error('CLI stderr:', stderr);
            parseAndReportErrors(stderr, cwd);
        } else {
            if (diagnosticCollection) {
                diagnosticCollection.clear();
            }
        }
        
        const raw: RequestShowRaw = JSON.parse(stdout);
        const output: RequestShowOutput = {
            name: raw.Request,
            auth: raw.Auth,
        };
        return output;
    } catch (error) {
        console.error('Failed to show request:', error);
        logCliError('Failed to show request', error);

        if (error instanceof Error && 'stderr' in error) {
            parseAndReportErrors((error as ExecError).stderr || '', getCliCommand().cwd);
        }

        throw new Error(`Failed to show request: ${getErrorMessage(error)}`);
    }
}

export interface ExecuteRequestOptions {
    requestName: string;
    sourceDirectory?: string;
    environment?: string;
    variables?: Record<string, string>;
}

export interface RequestExecutionResult {
    request_name: string;
    method: string;
    url: string;
    status: number;
    elapsed_ms: number;
    request_headers: Record<string, string>;
    response_headers: Record<string, string>;
    body: string;
}

export interface ExecuteRequestResult {
    results: RequestExecutionResult[];
    stderr?: string;
}

export async function executeRequest(options: ExecuteRequestOptions): Promise<ExecuteRequestResult> {
    try {
        const { executable, args: baseArgs, cwd } = getCliCommand();
        const args = [...baseArgs, 'request', 'run', '-n', options.requestName];

        if (options.sourceDirectory) {
            args.push('-s', options.sourceDirectory);
        }
        if (options.environment) {
            args.push('-e', options.environment);
        }

        if (options.variables) {
            Object.entries(options.variables).forEach(([key, value]) => {
                args.push('-v', `${key}=${value}`);
            });
        }

        args.push('-o', 'json');
        
        const fullCommand = `${executable} ${args.join(' ')}`;
        
        logCliExecution(fullCommand, cwd);
        console.log(`Executing request: ${fullCommand}`);
        if (cwd) {
            console.log(`Working directory: ${cwd}`);
        }
        
        const { stdout, stderr } = await spawnAsync(executable, args, { cwd });
        
        if (stderr) {
            parseAndReportErrors(stderr, cwd);
        } else {
            if (diagnosticCollection) {
                diagnosticCollection.clear();
            }
        }

        try {
            const jsonResult = JSON.parse(stdout);
            return {
                results: jsonResult.results || [],
                stderr: stderr || undefined
            };
        } catch (parseError) {
            console.error('Failed to parse JSON output:', parseError);
            throw new Error(`Failed to parse JSON output: ${parseError instanceof Error ? parseError.message : 'Unknown error'}`);
        }
    } catch (error) {
        console.error('Failed to execute request:', error);
        logCliError('Failed to execute request', error);
        
        if (error instanceof Error && 'stdout' in error && 'stderr' in error) {
            const execError = error as { stdout?: string; stderr?: string };
            
            if (execError.stderr) {
                parseAndReportErrors(execError.stderr, getCliCommand().cwd);
            }

            try {
                const jsonResult = JSON.parse(execError.stdout || '');
                return {
                    results: jsonResult.results || [],
                    stderr: execError.stderr || error.message
                };
            } catch {
                return {
                    results: [],
                    stderr: execError.stderr || error.message
                };
            }
        }
        throw new Error(`Failed to execute request: ${getErrorMessage(error)}`);
    }
}
