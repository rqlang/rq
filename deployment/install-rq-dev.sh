#!/usr/bin/env bash

set -euo pipefail

REPO="rqlang/rq"
WORKFLOW="release_cd.yaml"

BIN_NAME="rq-dev"
INSTALL_DIR=""

info()  { printf '[INFO ] %s\n' "$*" >&2; }
warn()  { printf '[WARN ] %s\n' "$*" >&2; }
error() { printf '[ERROR] %s\n' "$*" >&2; exit 1; }

usage() {
  cat <<EOF
Usage: $0 [OPTIONS]

Downloads the rq CLI from the latest successful dev build on GitHub Actions
and installs it next to the released rq without replacing it.

Options:
  -n, --name NAME         Name of the installed binary (default: rq-dev).
  -d, --install-dir DIR   Directory to install the binary into.
                          Overrides the default location logic.
  -h, --help              Show this help and exit.

Requires:
  - gh CLI installed and authenticated (gh auth login)
EOF
}

resolve_artifact_name() {
  local os_name
  local arch
  os_name="$(uname -s)"
  arch="$(uname -m)"

  case "${os_name}" in
    Linux)
      case "${arch}" in
        x86_64|amd64) printf 'rq-linux-x86_64' ;;
        aarch64|arm64) printf 'rq-linux-aarch64' ;;
        *) error "Unsupported Linux architecture: ${arch}. Dev builds exist for x86_64 and aarch64 only." ;;
      esac
      ;;
    Darwin)
      if [[ "${arch}" == "arm64" ]]; then
        printf 'rq-macos-aarch64'
      else
        printf 'rq-macos-x86_64'
      fi
      ;;
    *)
      error "Unsupported OS: ${os_name}. This installer supports Linux and macOS only."
      ;;
  esac
}

resolve_install_root() {
  if [[ -n "${INSTALL_DIR}" ]]; then
    info "Using custom install directory: ${INSTALL_DIR}"
    printf '%s' "${INSTALL_DIR}"
  elif [[ "${EUID:-$(id -u)}" -eq 0 ]]; then
    printf '/usr/local/bin'
  else
    info "Not running as root. Installing to ${HOME}/.local/bin instead of /usr/local/bin."
    printf '%s' "${HOME}/.local/bin"
  fi
}

latest_run_id() {
  local run_id
  if ! run_id="$(gh api "repos/${REPO}/actions/workflows/${WORKFLOW}/runs?status=success&per_page=1" \
    --jq '.workflow_runs[0].id // empty')"; then
    error "Failed to query the runs of ${WORKFLOW} in ${REPO}. Check that gh is authenticated (gh auth status)."
  fi
  [[ -n "${run_id}" ]] || error "No successful runs of ${WORKFLOW} found in ${REPO}."
  printf '%s' "${run_id}"
}

main() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      -n|--name)
        [[ $# -ge 2 ]] || error "Missing value for $1"
        BIN_NAME="$2"
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

  [[ -n "${BIN_NAME}" && "${BIN_NAME}" != */* ]] || error "Invalid binary name '${BIN_NAME}'."

  command -v gh >/dev/null 2>&1 || error "GitHub CLI (gh) not found in PATH. Install it from https://cli.github.com/ and run gh auth login."

  ARTIFACT_NAME="$(resolve_artifact_name)"
  info "Detected platform artifact: ${ARTIFACT_NAME}"

  INSTALL_ROOT="$(resolve_install_root)"
  INSTALL_BIN="${INSTALL_ROOT}/${BIN_NAME}"

  if [[ ! -d "${INSTALL_ROOT}" ]]; then
    info "Creating install directory: ${INSTALL_ROOT}"
    mkdir -p "${INSTALL_ROOT}" 2>/dev/null \
      || error "Cannot create ${INSTALL_ROOT}. Run the script with sudo, or pick a directory you own with --install-dir."
  fi

  if [[ ! -w "${INSTALL_ROOT}" ]]; then
    error "Cannot write to ${INSTALL_ROOT}. Run the script with sudo, or pick a directory you own with --install-dir."
  fi

  RUN_ID="$(latest_run_id)"
  info "Using run ${RUN_ID} of ${WORKFLOW}."

  WORK_DIR="$(mktemp -d)"
  trap 'rm -rf "${WORK_DIR}"' EXIT

  info "Downloading artifact '${ARTIFACT_NAME}'."
  gh run download "${RUN_ID}" --name "${ARTIFACT_NAME}" --dir "${WORK_DIR}" -R "${REPO}" \
    || error "Failed to download artifact '${ARTIFACT_NAME}' from run ${RUN_ID}. It may have expired."

  DOWNLOAD_PATH="${WORK_DIR}/rq"
  if [[ ! -f "${DOWNLOAD_PATH}" ]]; then
    error "Artifact '${ARTIFACT_NAME}' has no rq binary. Contents: $(ls -A "${WORK_DIR}")"
  fi

  info "Copying ${ARTIFACT_NAME} to ${INSTALL_BIN}"
  STAGED_BIN="$(mktemp "${INSTALL_ROOT}/.${BIN_NAME}.XXXXXX")"
  trap 'rm -rf "${WORK_DIR}" "${STAGED_BIN}"' EXIT
  install -m 755 "${DOWNLOAD_PATH}" "${STAGED_BIN}"

  if ! VERSION_OUTPUT="$("${STAGED_BIN}" --version 2>&1)"; then
    error "The downloaded '${ARTIFACT_NAME}' does not run on this system, so nothing was installed. On Linux it needs glibc 2.34 or newer and OpenSSL 3 (libssl.so.3). Output: ${VERSION_OUTPUT}"
  fi

  info "Downloaded binary runs: ${VERSION_OUTPUT}"
  mv -f "${STAGED_BIN}" "${INSTALL_BIN}"

  case ":${PATH}:" in
    *:"${INSTALL_ROOT}":*)
      ;;
    *)
      warn "${INSTALL_ROOT} is not in your PATH."
      warn "Add the following line to your shell profile (e.g., ~/.bashrc, ~/.zshrc, or ~/.bash_profile):"
      warn "  export PATH=\"${INSTALL_ROOT}:\$PATH\""
      ;;
  esac

  printf 'Success: Installed %s to %s\n' "${VERSION_OUTPUT}" "${INSTALL_BIN}" >&2
}

main "$@"
