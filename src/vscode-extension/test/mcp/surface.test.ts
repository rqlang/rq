import { buildGenerateRqPrompt, render, RESOURCE_BODIES, surface } from '../../src/mcp/surface';

describe('shared MCP surface', () => {
    it('declares the authoring tools', () => {
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

    it('covers the guidance behind each lint rule in the idioms guide', () => {
        const idioms = RESOURCE_BODIES[surface.resources['idioms'].uri];
        for (const needle of [
            '[required(',
            'Only introduce an `ep` block',
            'Always use relative import paths',
            'name requests after the verb alone',
            'For write actions, include a body',
            'JSON body syntax: always use the `${...}` prefix',
            'ep users<users_base>',
            'Never put a credential literal in a `.rq` file',
            'extending the same template declare identically belongs on the template',
            'Never re-declare on a child what the template already gives it'
        ]) {
            expect(idioms).toContain(needle);
        }
    });

    it('covers the core constructs in the language definition', () => {
        const definition = RESOURCE_BODIES[surface.resources['language-definition'].uri];
        for (const needle of ['rq', 'ep', 'env', 'auth', 'let', 'import']) {
            expect(definition).toContain(needle);
        }
    });

    it('gives every resource uri an authority and a path', () => {
        for (const definition of Object.values(surface.resources)) {
            expect(definition.uri).toMatch(/^rqlang:\/\/[^/]+\/.+$/);
        }
    });
});

describe('server instructions', () => {
    it('advertises both resource URIs with no unresolved placeholders', () => {
        const target = render(surface.instructions);
        expect(target).not.toMatch(/\{(language_definition_uri|idioms_uri)\}/);
        expect(target).toContain(surface.resources['language-definition'].uri);
        expect(target).toContain(surface.resources['idioms'].uri);
    });

    it('points clients without resource support at the reference tool', () => {
        expect(render(surface.instructions)).toContain('get_rq_reference');
    });

    it('describes the one endpoint per file layout', () => {
        const target = render(surface.instructions);
        expect(target).toContain('one `ep` per file');
        expect(target).toContain('once per file');
    });

    it('forbids lint suppressions the user did not ask for', () => {
        expect(render(surface.instructions)).toContain('never add an `rq-lint-ignore` comment unless the user explicitly asks');
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

    it('references both rqlang documents', () => {
        const target = buildGenerateRqPrompt('list users');
        expect(target).toContain(surface.resources['language-definition'].uri);
        expect(target).toContain(surface.resources['idioms'].uri);
    });

    it('points at the reference tool', () => {
        expect(buildGenerateRqPrompt('list users')).toContain('get_rq_reference');
    });

    it('forbids unsolicited CLI suggestions', () => {
        const target = buildGenerateRqPrompt('list users');
        expect(target).toContain('Do not suggest CLI commands');
        expect(target).toContain('explicitly asks');
    });

    it('requires one endpoint per file', () => {
        const target = buildGenerateRqPrompt('users and widgets CRUD');
        expect(target).toContain('Each `ep` goes in its own file');
        expect(target).toContain('Never put two `ep` blocks in the same file');
    });

    it('drives the validate and lint loop per file', () => {
        const target = buildGenerateRqPrompt('list users');
        expect(target).toContain('Validate each file separately');
        expect(target).toContain('Lint each file separately');
        expect(target).toContain('every file returns `ok: true`');
    });

    it('explains that splitting files clears the endpoint rule', () => {
        const target = buildGenerateRqPrompt('list users');
        expect(target).toContain('multiple_endpoints_per_file');
        expect(target).toContain('Never drop content the user asked for');
    });

    it('asks for one code block per file', () => {
        expect(buildGenerateRqPrompt('list users')).toContain('own markdown code block with its filename');
    });

    it('forbids lint suppressions the user did not ask for', () => {
        expect(buildGenerateRqPrompt('list users')).toContain('never add an `rq-lint-ignore` comment unless the user explicitly asks');
    });

    it('passes the workspace path to the validate step', () => {
        const target = buildGenerateRqPrompt('list users', '/repo').split('\n').find(line => line.includes('validate_rq'));
        expect(target).toContain('workspace_path');
    });
});
