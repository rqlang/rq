import * as vscode from 'vscode';

export class Logger {
    private constructor(
        private outputChannel: vscode.OutputChannel,
        private debugEnabled: boolean
    ) {}

    static init(outputChannel: vscode.OutputChannel): Logger {
        const debug = vscode.workspace.getConfiguration('rq').get<boolean>('debugLogging', false);
        return new Logger(outputChannel, debug);
    }

    log(message: string): void {
        this.outputChannel.appendLine(message);
    }

    debug(message: string): void {
        if (!this.debugEnabled) { return; }
        this.outputChannel.appendLine(message);
    }

    static redactAuthValue(value: string): string {
        const spaceIdx = value.indexOf(' ');
        if (spaceIdx !== -1) {
            const scheme = value.slice(0, spaceIdx);
            return `${scheme} [REDACTED]`;
        }
        return '[REDACTED]';
    }
}
