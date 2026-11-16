export type JsonNode =
    | { type: 'null' | 'boolean' | 'number'; text: string }
    | { type: 'string'; value: string }
    | { type: 'array'; items: JsonNode[] }
    | { type: 'object'; entries: [string, JsonNode][] };

const whitespacePattern = /[ \t\n\r]*/y;
const stringPattern = /"(?:[^"\\]|\\.)*"/y;
const literalPattern = /-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?|true|false|null/y;

export function parseJsonTree(text: string): JsonNode | undefined {
    try {
        return new JsonTreeParser(text).parse();
    } catch {
        return undefined;
    }
}

class JsonTreeParser {
    private position = 0;

    constructor(private readonly text: string) {}

    parse(): JsonNode {
        const node = this.value();
        this.skipWhitespace();
        if (this.position !== this.text.length) {
            throw new SyntaxError(`Unexpected content at ${this.position}`);
        }
        return node;
    }

    private value(): JsonNode {
        this.skipWhitespace();
        switch (this.text[this.position]) {
            case '{':
                return this.object();
            case '[':
                return this.array();
            case '"':
                return { type: 'string', value: this.string() };
            default:
                return this.literal();
        }
    }

    private object(): JsonNode {
        const entries: [string, JsonNode][] = [];
        this.position++;
        if (this.consume('}')) {
            return { type: 'object', entries };
        }
        do {
            this.skipWhitespace();
            const key = this.string();
            this.expect(':');
            entries.push([key, this.value()]);
        } while (this.consume(','));
        this.expect('}');
        return { type: 'object', entries };
    }

    private array(): JsonNode {
        const items: JsonNode[] = [];
        this.position++;
        if (this.consume(']')) {
            return { type: 'array', items };
        }
        do {
            items.push(this.value());
        } while (this.consume(','));
        this.expect(']');
        return { type: 'array', items };
    }

    private string(): string {
        return JSON.parse(this.match(stringPattern));
    }

    private literal(): JsonNode {
        const text = this.match(literalPattern);
        const type = text === 'null' ? 'null' : text === 'true' || text === 'false' ? 'boolean' : 'number';
        return { type, text };
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
