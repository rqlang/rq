import * as vscode from 'vscode';
import { ExplorerState } from '../src/explorerState';

export class FakeMemento implements vscode.Memento {
    private readonly values = new Map<string, unknown>();

    keys(): readonly string[] {
        return [...this.values.keys()];
    }

    get<T>(key: string, defaultValue?: T): T | undefined {
        return this.values.has(key) ? this.values.get(key) as T : defaultValue;
    }

    update(key: string, value: unknown): Thenable<void> {
        this.values.set(key, value);
        return Promise.resolve();
    }
}

export function createExplorerState(memento: vscode.Memento = new FakeMemento()): ExplorerState {
    return new ExplorerState(memento, 'test.explorer');
}
