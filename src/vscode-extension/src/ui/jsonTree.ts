export type JsonNode = string | [string] | JsonContainer;
type JsonContainer = ['a' | 'o', ...JsonNode[]];

const whitespacePattern = /[ \t\n\r]*/y;
const stringPattern = /"[^"\\]*(?:\\.[^"\\]*)*"/y;
const literalPattern = /-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?|true|false|null/y;

export function parseJsonTree(text: string): JsonNode | undefined {
    try {
        return new JsonTreeParser(text).parse();
    } catch {
        return undefined;
    }
}

export function serializeJsonTree(tree: JsonNode): string {
    const output: string[] = [];
    const pending: (JsonNode | { text: string })[] = [tree];
    for (let piece = pending.pop(); piece !== undefined; piece = pending.pop()) {
        if (typeof piece === 'string' || Array.isArray(piece) && piece.length === 1) {
            output.push(JSON.stringify(piece));
        } else if (!Array.isArray(piece)) {
            output.push(piece.text);
        } else {
            output.push(`[${JSON.stringify(piece[0])}`);
            pending.push({ text: ']' });
            for (let index = piece.length - 1; index > 0; index--) {
                pending.push(piece[index], { text: ',' });
            }
        }
    }
    return output.join('');
}

class JsonTreeParser {
    private position = 0;
    private readonly open: JsonContainer[] = [];

    constructor(private readonly text: string) {}

    parse(): JsonNode {
        let node = this.startValue();
        for (;;) {
            const container = this.open[this.open.length - 1];
            if (container === undefined) {
                return this.finish(node);
            } else if (node === undefined) {
                node = this.consume(closerOf(container)) ? this.open.pop() : this.startEntry(container);
            } else {
                container.push(node);
                node = this.consume(',') ? this.startEntry(container) : this.close(container);
            }
        }
    }

    private startEntry(container: JsonContainer): JsonNode | undefined {
        if (container[0] === 'o') {
            this.skipWhitespace();
            container.push(this.string());
            this.expect(':');
        }
        return this.startValue();
    }

    private startValue(): JsonNode | undefined {
        this.skipWhitespace();
        switch (this.text[this.position]) {
            case '[':
                this.position++;
                this.open.push(['a']);
                return undefined;
            case '{':
                this.position++;
                this.open.push(['o']);
                return undefined;
            case '"':
                return this.string();
            default:
                return [this.match(literalPattern)];
        }
    }

    private close(container: JsonContainer): JsonNode | undefined {
        this.expect(closerOf(container));
        return this.open.pop();
    }

    private finish(node: JsonNode | undefined): JsonNode {
        this.skipWhitespace();
        if (node === undefined || this.position !== this.text.length) {
            throw new SyntaxError(`Unexpected content at ${this.position}`);
        }
        return node;
    }

    private string(): string {
        return JSON.parse(this.match(stringPattern));
    }

    private match(pattern: RegExp): string {
        pattern.lastIndex = this.position;
        const match = pattern.exec(this.text);
        if (!match) {
            throw new SyntaxError(`Unexpected token at ${this.position}`);
        }
        this.position = pattern.lastIndex;
        return match[0];
    }

    private consume(char: string): boolean {
        this.skipWhitespace();
        if (this.text[this.position] !== char) {
            return false;
        }
        this.position++;
        return true;
    }

    private expect(char: string): void {
        if (!this.consume(char)) {
            throw new SyntaxError(`Expected '${char}' at ${this.position}`);
        }
    }

    private skipWhitespace(): void {
        this.match(whitespacePattern);
    }
}

function closerOf(container: JsonContainer): string {
    return container[0] === 'a' ? ']' : '}';
}
