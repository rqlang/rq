import surfaceJson from '../../../rq-mcp/surface.json';
import languageDefinitionMd from '../../../../docs/LANGUAGE_DEFINITION.md';
import idiomsMd from '../../../../docs/RQLANG_IDIOMS.md';

export interface ResourceDefinition {
    uri: string;
    title: string;
    file: string;
    description: string;
}

export const surface = surfaceJson as {
    instructions: string;
    tools: Record<string, { description: string }>;
    resources: Record<string, ResourceDefinition>;
    prompt: {
        description: string;
        header: string;
        workspace_clause_with: string;
        workspace_clause_without: string;
        steps: Record<string, string>;
        constraints: string;
    };
};

export const RESOURCE_BODIES: Record<string, string> = {
    [surface.resources['language-definition'].uri]: languageDefinitionMd,
    [surface.resources['idioms'].uri]: idiomsMd
};

export function render(template: string): string {
    return template
        .replace(/\{language_definition_uri\}/g, surface.resources['language-definition'].uri)
        .replace(/\{idioms_uri\}/g, surface.resources['idioms'].uri);
}

export function buildGenerateRqPrompt(intent: string, workspacePath?: string): string {
    const steps = surface.prompt.steps;
    const clause = workspacePath
        ? surface.prompt.workspace_clause_with.replace('{path}', workspacePath)
        : surface.prompt.workspace_clause_without;

    const ordered = [
        steps.read_resources,
        workspacePath
            ? steps.list_requests_with_workspace.replace('{path}', workspacePath)
            : steps.list_requests_default,
        steps.file_layout,
        steps.draft,
        steps.validate,
        steps.lint,
        steps.present
    ];

    const workflow = ordered
        .map((step, index) => `${index + 1}. ${render(step).replace(/\{workspace_clause\}/g, clause)}\n`)
        .join('');

    const header = surface.prompt.header.replace('{intent}', intent);
    return `${header}${workflow}\n${render(surface.prompt.constraints)}`;
}
