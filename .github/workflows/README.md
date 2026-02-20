# Release Workflows

This directory contains GitHub Actions workflows for building and releasing the `rq` CLI tool.

## Workflows

### 1. CLI Release Production (`cli_release_prod.yaml`)

**Trigger:** Manual creation of a GitHub Release

**Purpose:** Builds and uploads compiled binaries to an existing GitHub Release.

**Versioning:** Extracted from git tag
- Tag `v1.0.0` → Version `1.0.0`
- Tag `v0.1.5` → Version `0.1.5`
- Full semantic version from tag

**Platforms:**
- Linux x86_64
- Windows x86_64
- macOS x86_64
- macOS ARM64 (Apple Silicon)

**Artifacts:** Binary executables uploaded to the Release

**Usage:**
1. Go to GitHub repository → Releases → "Draft a new release"
2. Create a tag (e.g., `v1.0.0`) and set release title/description
3. Publish the release
4. Workflow automatically builds and uploads binaries to the release

### 2. Release CD (`release_cd.yaml`)

**Trigger:** Push to `main` branch

**Purpose:** Automated dev builds of both CLI (Windows) and VS Code extension.

**Versioning:** `{base}-dev.{commits_since_tag}`
- Finds the latest `v*` git tag (e.g., `v0.1.0` → `0.1.0`)
- If no tag exists, defaults to `0.0.0`
- Counts commits since that tag as the build number
- Final version: `0.1.0-dev.5`

**Jobs:**
- **Build CLI (Windows x86_64):** Cargo build, zip archive, upload artifact
- **Build VS Code Extension:** npm ci, vsce package, upload VSIX artifact

**Artifacts:** CLI zip and VSIX uploaded as workflow artifacts (not releases)

## Version Management

### Release Versions (Production)

Release versions come from the tag created when publishing a GitHub Release. The workflow is triggered when you publish a release in GitHub UI.

### Dev Versions (CD Builds)

Dev versions are derived automatically from git tags. When you push to `main`, the CD pipeline finds the latest `v*` tag and counts commits since it:

```
Latest tag: v0.1.0, 5 commits since → version 0.1.0-dev.5
No tags exist → version 0.0.0-dev.1
```

No VERSION file is needed.

### Local Development Version

For local development builds, the version remains `0.0.0` as specified in `cli/Cargo.toml`. The workflows automatically update this during CI/CD builds.

## Manual Local Build

```bash
cd cli
cargo build --release
```

## Release Process

1. **Prepare:** Ensure all changes are merged and tested
2. **Create Release:** Go to GitHub → Releases → "Draft a new release"
3. **Set Version:** Create tag (e.g., `v1.0.0`), add title and release notes
4. **Publish:** Click "Publish release"
5. **Monitor:** Check GitHub Actions for build status
6. **Verify:** Once workflow completes, download and test release binaries

## Notes

- The run number automatically increments with each workflow execution
- Semantic versioning is preserved with the `-dev.X` suffix for CD builds
- Release builds are clean versions suitable for distribution
- All builds include automated tests before artifact creation
