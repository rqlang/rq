import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { describe, it } from 'node:test';
import { fileURLToPath } from 'node:url';

const packageDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const pluginDir = path.resolve(packageDir, '../claude-plugin');

function readJson(file) {
    return JSON.parse(fs.readFileSync(file, 'utf8'));
}

function frontmatter(file) {
    const match = /^---\n([\s\S]*?)\n---\n/.exec(fs.readFileSync(file, 'utf8'));
    assert.ok(match, `${file} has no frontmatter`);
    return Object.fromEntries(match[1].split('\n').map(line => {
        const separator = line.indexOf(':');
        return [line.slice(0, separator).trim(), line.slice(separator + 1).trim()];
    }));
}

const manifest = readJson(path.join(pluginDir, '.claude-plugin/plugin.json'));
const marketplace = readJson(path.join(pluginDir, '.claude-plugin/marketplace.json'));
const servers = readJson(path.join(pluginDir, '.mcp.json')).mcpServers;
const packageJson = readJson(path.join(packageDir, 'package.json'));
const surface = readJson(path.join(packageDir, 'surface.json'));
const launcher = path.join(pluginDir, 'scripts/npx.mjs');
const skillsDir = path.join(pluginDir, 'skills');
const skills = fs.readdirSync(skillsDir).map(name => ({ name, file: path.join(skillsDir, name, 'SKILL.md') }));

describe('claude plugin manifest', () => {
    it('names the plugin and keeps the version the release replaces', () => {
        assert.equal(manifest.name, 'rqlang');
        assert.equal(manifest.version, '0.0.0');
    });

    it('ships the license it declares', () => {
        assert.equal(manifest.license, 'MIT');
        assert.match(fs.readFileSync(path.join(pluginDir, 'LICENSE'), 'utf8'), /^MIT License\n/);
    });

    it('has a description', () => {
        assert.ok(manifest.description.length > 0);
    });
});

describe('claude plugin marketplace', () => {
    it('lists only this plugin from the repository root', () => {
        assert.deepEqual(marketplace.plugins.map(plugin => [plugin.name, plugin.source]), [[manifest.name, './']]);
    });

    it('leaves the version to the plugin manifest', () => {
        assert.equal(marketplace.plugins[0].version, undefined);
    });
});

describe('claude plugin mcp server', () => {
    it('declares exactly one server', () => {
        assert.deepEqual(Object.keys(servers), ['rq']);
    });

    it('starts this package at the plugin version through the bundled npx launcher', () => {
        assert.equal(servers.rq.command, 'node');
        assert.deepEqual(servers.rq.args, [
            '${CLAUDE_PLUGIN_ROOT}/scripts/npx.mjs', '-y', `${packageJson.name}@${manifest.version}`
        ]);
    });

    it('passes its arguments to npx', () => {
        const target = spawnSync(process.execPath, [launcher, '--version'], { encoding: 'utf8' });
        assert.equal(target.status, 0, target.stderr);
        assert.match(target.stdout.trim(), /^\d+\.\d+\.\d+$/);
    });

    it('reports a failing npx run through its exit code', () => {
        const target = spawnSync(process.execPath, [launcher, '--no-install', '--package', 'rq-mcp-missing-launcher-probe', '--', 'missing'], {
            encoding: 'utf8',
            env: { ...process.env, npm_config_offline: 'true' }
        });
        assert.notEqual(target.status, 0);
    });

    it('starts the only bin the package exposes', () => {
        assert.equal(Object.keys(packageJson.bin).length, 1);
    });
});

describe('claude plugin skills', () => {
    it('ships at least one skill', () => {
        assert.ok(skills.length > 0);
    });

    it('names each skill after its folder and describes it', () => {
        for (const skill of skills) {
            const target = frontmatter(skill.file);
            assert.equal(target.name, skill.name);
            assert.ok(target.description.length > 0, `${skill.name} has no description`);
        }
    });

    it('only calls tools the server declares', () => {
        for (const skill of skills) {
            const called = fs.readFileSync(skill.file, 'utf8').match(/`[a-z]+_rq[a-z_]*`|`list_requests`/g) ?? [];
            for (const tool of called) {
                assert.ok(tool.slice(1, -1) in surface.tools, `${skill.name} calls unknown tool ${tool}`);
            }
        }
    });

    it('checks drafts against the folder they are written to', () => {
        for (const skill of skills) {
            assert.match(fs.readFileSync(skill.file, 'utf8'), /`workspace_path`/, skill.name);
        }
    });
});
