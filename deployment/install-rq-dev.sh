#!/usr/bin/env bash

set -euo pipefail

REPO="rqlang/rq"
WORKFLOW="release_cd.yaml"
ARTIFACT_NAME="rq-macos-x86_64"

PREFERRED_ROOT="/usr/local/bin"
FALLBACK_ROOT="${HOME}/.local/bin"

info()  { printf '[INFO ] %s\n' "$*" >&2; }
warn()  { printf '[WARN ] %s\n' "$*" >&2; }
error() { printf '[ERROR] %s\n' "$*" >&2; exit 1; }

usage() {
  cat <<EOF
Usage: $0 [OPTIONS]

Downloads the latest rq CLI dev build artifact from GitHub Actions
and installs it.

Options:
  -d, --install-dir DIR   Directory to install the rq binary into.
                          Overrides the default location logic.
  -h, --help              Show this help and exit.

Requires:
  - gh CLI installed and authenticated (gh auth login)
EOF
}

INSTALL_DIR=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --release-tag)
      # Ignored in dev script but accepted for compatibility with prod script invocation
      shift 2
      ;;
    -d|--install-dir)
      [[ $# -ge 2 ]] || error "Missing value for $1"
      INSTALL_DIR="$2"
      shift 2
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

command -v gh >/dev/null 2>&1 || error "GitHub CLI (gh) not found in PATH. Install from https://cli.github.com/ and authenticate with gh auth login."

RUN_ID="$(get_latest_run_id)"

if [[ -n "${INSTALL_DIR}" ]]; then
  INSTALL_ROOT="${INSTALL_DIR}"
  info "Using custom install directory: ${INSTALL_ROOT}"
else
  EFFECTIVE_UID="${EUID:-$(id -u)}"
  if [[ "${EFFECTIVE_UID}" -eq 0 ]]; then
    INSTALL_ROOT="${PREFERRED_ROOT}"
  else
    INSTALL_ROOT="${FALLBACK_ROOT}"
    info "Not running as root. Installing to ${INSTALL_ROOT} instead of ${PREFERRED_ROOT}."
  fi
fi

if [[ ! -d "${INSTALL_ROOT}" ]]; then
  info "Creating install directory: ${INSTALL_ROOT}"
  mkdir -p "${INSTALL_ROOT}"
fi

INSTALL_BIN="${INSTALL_ROOT}/rq"

WORK_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/rq-dev-XXXXXX")"

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
  info "Expanding ${zip} ..."
  unzip -o -q "${zip}"
done

# 'mapfile' is bash 4+, not available in macos bash 3.2 by default
# Convert find output into array using while read loop
RQ_CANDIDATES=()
while IFS= read -r file; do
    RQ_CANDIDATES+=("$file")
done < <(find "${WORK_ROOT}" -name 'rq' -type f 2>/dev/null)

if [[ ${#RQ_CANDIDATES[@]} -eq 0 ]]; then
  error "Did not find rq binary inside artifact '${ARTIFACT_NAME}'. Contents:
$(find "${WORK_ROOT}" -type f)"
fi

if [[ ${#RQ_CANDIDATES[@]} -gt 1 ]]; then
  warn "Multiple rq binaries found; using first one:"
  printf '  %s\n' "${RQ_CANDIDATES[@]}" >&2
fi

SOURCE_BIN="${RQ_CANDIDATES[0]}"
chmod +x "${SOURCE_BIN}"
info "Using rq at ${SOURCE_BIN}"

if [[ -f "${INSTALL_BIN}" ]]; then
  info "Backing up existing rq to rq.bck"
  cp "${INSTALL_BIN}" "${INSTALL_BIN}.bck"
fi

info "Copying new rq to ${INSTALL_BIN}"
cp "${SOURCE_BIN}" "${INSTALL_BIN}"

if [[ ! -x "${INSTALL_BIN}" ]]; then
  error "Failed to install rq to ${INSTALL_BIN}"
fi

popd >/dev/null

case ":${PATH}:" in
  *:"${INSTALL_ROOT}":*)
    ;;
  *)
    warn "${INSTALL_ROOT} is not in your PATH."
    warn "Add the following line to your shell profile (e.g., ~/.bashrc, ~/.zshrc, or ~/.bash_profile):"
    warn "  export PATH=\"${INSTALL_ROOT}:\$PATH\""
    ;;
esac

printf 'Success: Installed rq to %s\n' "${INSTALL_BIN}" >&2
