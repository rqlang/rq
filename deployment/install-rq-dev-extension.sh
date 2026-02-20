#!/usr/bin/env bash

set -euo pipefail

REPO="rqlang/rq"
WORKFLOW="release_cd.yaml"
ARTIFACT_PATTERN="rq-language-extension-"

RUN_ID=""
FORCE=false

info()  { printf '[INFO ] %s\n' "$*" >&2; }
warn()  { printf '[WARN ] %s\n' "$*" >&2; }
error() { printf '[ERROR] %s\n' "$*" >&2; exit 1; }

usage() {
  cat <<EOF
Usage: $0 [OPTIONS]

Downloads the rq-language-extension VSIX artifact from GitHub Actions
and installs it into VS Code.

Options:
  -r, --run-id ID   GitHub Actions workflow run ID. If omitted, the
                     latest successful run is auto-detected.
  -f, --force       Force reinstall even if already installed.
  -h, --help        Show this help and exit.

Requires:
  - gh CLI installed and authenticated (gh auth login)
  - VS Code CLI (code) available in PATH
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    -r|--run-id)
      [[ $# -ge 2 ]] || error "Missing value for $1"
      RUN_ID="$2"
      shift 2
      ;;
    -f|--force)
      FORCE=true
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      error "Unknown argument: $1"
      ;;
  esac
done

get_latest_run_id() {
  info "Auto-detecting latest successful workflow run (repo=${REPO} workflow=${WORKFLOW})"

  local endpoint="repos/${REPO}/actions/workflows/${WORKFLOW}/runs?status=success&per_page=1"
  local run_id

  run_id="$(gh api "$endpoint" --jq '.workflow_runs[0].id' 2>/dev/null)" || true

  if [[ -z "${run_id}" ]]; then
    local json_raw
    json_raw="$(gh api "$endpoint" 2>/dev/null)" || true
    if [[ -n "${json_raw}" ]]; then
      run_id="$(printf '%s' "${json_raw}" | sed -n 's/.*"id"[[:space:]]*:[[:space:]]*\([0-9]*\).*/\1/p' | head -n1)"
    fi
  fi

  if [[ -z "${run_id}" ]]; then
    error "No successful workflow runs found for repo '${REPO}' (workflow: ${WORKFLOW})"
  fi

  info "Using latest run id: ${run_id}"
  printf '%s' "${run_id}"
}

command -v gh   >/dev/null 2>&1 || error "GitHub CLI (gh) not found in PATH. Install from https://cli.github.com/ and authenticate with gh auth login."
command -v code >/dev/null 2>&1 || error "VS Code CLI (code) not found in PATH. Ensure VS Code is installed and added to PATH."

if [[ -z "${RUN_ID}" ]]; then
  # Fetch latest run_id using gh run list
  # We filter by workflow and status=success, limit 1
  # The run id is the 7th column in the table output by default if formatted, but let's use json
  # gh run list --workflow release_cd.yaml --status success --limit 1 --json databaseId --jq '.[0].databaseId'
  
  RUN_ID=$(gh run list --workflow release_cd.yaml --status success --limit 1 --json databaseId --jq '.[0].databaseId')
  
  if [[ -z "${RUN_ID}" || "${RUN_ID}" == "null" ]]; then
     error "Could not find any successful run for workflow release_cd.yaml"
  fi
  info "Using latest run id: ${RUN_ID}"
fi

info "Fetching artifacts list for run ${RUN_ID} ..."

# We will let 'gh run download' do the artifact selection for us if possible,
# or list artifacts first properly.
# But 'gh run download' downloads EVERYTHING if we don't specify name.
# We want the one starting with rq-language-extension-...
# Let's find the exact name first.

ARTIFACT_NAME=$(gh api "repos/rqlang/rq/actions/runs/${RUN_ID}/artifacts" --jq '.artifacts[] | select(.name | startswith("rq-language-extension-")) | .name' | head -n1)

if [[ -z "${ARTIFACT_NAME}" ]]; then
    error "No artifact found starting with 'rq-language-extension-' in run ${RUN_ID}"
fi
info "Selected artifact: ${ARTIFACT_NAME}"

WORK_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/rq-extension-XXXXXX")"

cleanup() {
  rm -rf "${WORK_ROOT}" 2>/dev/null || warn "Temp cleanup failed"
}
trap cleanup EXIT

pushd "${WORK_ROOT}" >/dev/null

info "Downloading artifact '${ARTIFACT_NAME}' for run ${RUN_ID} ..."
info "gh run download ${RUN_ID} --name ${ARTIFACT_NAME} --dir . -R ${REPO}"
gh run download "${RUN_ID}" --name "${ARTIFACT_NAME}" --dir . -R "${REPO}"

for zip in *.zip; do
  [[ -f "${zip}" ]] || continue
  EXTRACT_DIR="${WORK_ROOT}/${zip%.zip}"
  info "Expanding ${zip} ..."
  mkdir -p "${EXTRACT_DIR}"
  unzip -o -q "${zip}" -d "${EXTRACT_DIR}"
done

# Try to find vsix recursively if not immediately expanded or if zip contained folders
VSIX_CANDIDATES=()
while IFS= read -r file; do
  VSIX_CANDIDATES+=("$file")
done < <(find "${WORK_ROOT}" -name '*.vsix' -type f 2>/dev/null)

if [[ ${#VSIX_CANDIDATES[@]} -eq 0 ]]; then
  error "Did not find .vsix file inside artifact '${ARTIFACT_NAME}'. Contents:
$(find "${WORK_ROOT}" -type f)"
fi

if [[ ${#VSIX_CANDIDATES[@]} -gt 1 ]]; then
  warn "Multiple .vsix files found; using first one:"
  printf '  %s\n' "${VSIX_CANDIDATES[@]}" >&2
fi

VSIX_PATH="${VSIX_CANDIDATES[0]}"
info "Using VSIX at ${VSIX_PATH}"

CODE_ARGS=("--install-extension" "${VSIX_PATH}")
if [[ "${FORCE}" == true ]]; then
  CODE_ARGS+=("--force")
  info "Installing extension (force mode) ..."
else
  info "Installing extension ..."
fi

info "code ${CODE_ARGS[*]}"
code "${CODE_ARGS[@]}"

popd >/dev/null

printf 'Success: Installed extension from %s\n' "${VSIX_PATH}" >&2
