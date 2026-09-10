import { buildGenerateRqPrompt, render, RESOURCE_BODIES, surface } from '../../src/mcp/surface';

describe('shared MCP surface', () => {
    it('declares the same tools the Rust server serves', () => {
        expect(Object.keys(surface.tools).sort()).toEqual(
            ['get_rq_reference', 'lint_rq', 'list_requests', 'validate_rq']
        );
    });

    it('carries the markdown body for every declared resource', () => {
        for (const definition of Object.values(surface.resources)) {
            expect(RESOURCE_BODIES[definition.uri].length).toBeGreaterThan(1000);
        }
    });

    it('serves the idioms guide that backs the lint rules', () => {
        const idioms = RESOURCE_BODIES[surface.resources['idioms'].uri];
        expect(idioms).toContain('Never hand-write an `Authorization` header');
        expect(idioms).toContain('goes on the `ep`, not on each `rq`');
    });
});

describe('server instructions', () => {
    it('advertises both resource URIs with no unresolved placeholders', () => {
        const target = render(surface.instructions);
        expect(target).not.toMatch(/\{(language_definition_uri|idioms_uri)\}/);
        expect(target).toContain(surface.resources['language-definition'].uri);
        expect(target).toContain(surface.resources['idioms'].uri);
    });
});

describe('buildGenerateRqPrompt', () => {
    it('leaves no unresolved placeholders', () => {
        const target = buildGenerateRqPrompt('list users', '/repo');
        expect(target).not.toMatch(/\{(intent|path|workspace_clause|language_definition_uri|idioms_uri)\}/);
    });

    it('keeps rqlang double-brace interpolation intact in the constraints', () => {
        const target = buildGenerateRqPrompt('list users');
        expect(target).toContain('`{{name}}`');
    });

    it('embeds the intent', () => {
        expect(buildGenerateRqPrompt('list every widget')).toContain('> list every widget');
    });

    it('names the workspace path in the steps when one is given', () => {
        const target = buildGenerateRqPrompt('list users', '/repo');
        expect(target).toContain('path="/repo"');
        expect(target).toContain('`workspace_path="/repo"`');
    });

    it('falls back to the working directory wording without a workspace', () => {
        const target = buildGenerateRqPrompt('list users');
        expect(target).toContain('`workspace_path` set to the current working directory');
        expect(target).toContain('omit `path`');
    });

    it('numbers the workflow steps', () => {
        const target = buildGenerateRqPrompt('list users');
        expect(target).toContain('1. Read both rqlang documents before writing anything');
        expect(target).toContain('7. Present each file');
    });
});
