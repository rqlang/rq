#!/usr/bin/env bash

set -euo pipefail

OWNER="rqlang"
REPO="rq"
API_BASE="https://api.github.com/repos/${OWNER}/${REPO}"

RELEASE_TAG=""
INSTALL_DIR=""
RELEASE_JSON=""

info() {
  printf '[INFO ] %s\n' "$*" >&2
}

warn() {
  printf '[WARN ] %s\n' "$*" >&2
}

error() {
  printf '[ERROR] %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<EOF
Usage: $0 [OPTIONS]

Options:
  -r, --release-tag TAG   GitHub release tag to install (e.g. v0.4.0).
                          If omitted, the latest release is used.
  -d, --install-dir DIR   Directory to install the rq binary into.
                          Overrides the default location logic.
  -h, --help              Show this help and exit.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    -r|--release-tag)
      [[ $# -ge 2 ]] || error "Missing value for $1"
      RELEASE_TAG="$2"
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

OS_NAME="$(uname -s)"
ARCH="$(uname -m)"
ASSET_NAME=""

case "${OS_NAME}" in
  Linux)
    ASSET_NAME="rq-linux-x86_64"

    if [[ -r /etc/os-release ]]; then
      # shellcheck disable=SC1091
      . /etc/os-release
      if [[ "${ID:-}" == "ubuntu" && "${VERSION_ID:-}" == "22.04" ]]; then
        ASSET_NAME="rq-linux-ubuntu-22.04-x86_64"
      fi
    fi
    ;;
  Darwin)
    if [[ "${ARCH}" == "arm64" ]]; then
      ASSET_NAME="rq-macos-aarch64"
    else
      ASSET_NAME="rq-macos-x86_64"
    fi
    ;;
  *)
    error "Unsupported OS: ${OS_NAME}. This installer supports Linux and macOS only."
    ;;
esac

info "Detected platform: OS=${OS_NAME}, ARCH=${ARCH}, asset=${ASSET_NAME}"

if [[ -z "${RELEASE_TAG}" ]]; then
  info "Release tag not provided. Fetching latest release from GitHub (${OWNER}/${REPO})."

  CURL_ARGS=("-fsSL" "-H" "User-Agent: rq-installer-script")

  if ! RELEASE_JSON="$(curl "${CURL_ARGS[@]}" "${API_BASE}/releases/latest")"; then
    warn "Latest release endpoint returned an error. Falling back to releases list (including pre-releases)."

    if ! RELEASE_JSON="$(curl "${CURL_ARGS[@]}" "${API_BASE}/releases?per_page=1")"; then
      error "Failed to retrieve releases list from GitHub."
    fi
  fi

  RELEASE_TAG="$(printf '%s\n' "${RELEASE_JSON}" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n1)"
  if [[ -z "${RELEASE_TAG}" ]]; then
    error "Could not determine latest release tag from GitHub response."
  fi
else
  info "Using specified release tag '${RELEASE_TAG}'."

  CURL_ARGS=("-fsSL" "-H" "User-Agent: rq-installer-script")

  if ! RELEASE_JSON="$(curl "${CURL_ARGS[@]}" "${API_BASE}/releases/tags/${RELEASE_TAG}")"; then
    error "Failed to retrieve release '${RELEASE_TAG}' from GitHub."
  fi
fi

info "Installing release '${RELEASE_TAG}' from repository ${OWNER}/${REPO}."

ASSET_URL="$(printf '%s' "${RELEASE_JSON}" \
  | awk -v asset="${ASSET_NAME}" '
    BEGIN { RS = "," }
    /\/releases\/assets\// && /"url"/ {
      s = $0
      gsub(/.*"url"[[:space:]]*:[[:space:]]*"/, "", s)
      gsub(/".*/, "", s)
      candidate = s
    }
    /"name"/ && index($0, "\"" asset "\"") {
      print candidate
      exit
    }
  ')"

if [[ -z "${ASSET_URL}" ]]; then
  AVAILABLE_ASSETS_LIST="$(printf '%s\n' "${RELEASE_JSON}" | sed -n 's/.*"name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')"
  AVAILABLE_ASSETS="$(printf '%s\n' "${AVAILABLE_ASSETS_LIST}" | tr '\n' ', ' | sed 's/, $//')"
  error "Asset '${ASSET_NAME}' not found in release '${RELEASE_TAG}'. Available assets: ${AVAILABLE_ASSETS}"
fi

TMPDIR="${TMPDIR:-/tmp}"
DOWNLOAD_PATH="${TMPDIR}/rq_$$"

info "Downloading asset '${ASSET_NAME}' via API (${ASSET_URL})"

CURL_REDIRECT_ARGS=("-sS" "-w" "%{redirect_url}" "-o" "/dev/null"
  "-H" "User-Agent: rq-installer-script"
  "-H" "Accept: application/octet-stream")

SIGNED_URL="$(curl "${CURL_REDIRECT_ARGS[@]}" "${ASSET_URL}")"

if [[ -z "${SIGNED_URL}" ]]; then
  error "GitHub API did not return a redirect for asset '${ASSET_NAME}'. The release or asset may not exist."
fi

info "Downloading from signed URL..."
if ! curl -fSL -o "${DOWNLOAD_PATH}" "${SIGNED_URL}"; then
  error "Failed to download asset '${ASSET_NAME}' from GitHub."
fi

if [[ ! -f "${DOWNLOAD_PATH}" ]]; then
  error "Download appeared to succeed but file not found at ${DOWNLOAD_PATH}"
fi

chmod +x "${DOWNLOAD_PATH}"

# Decide installation root:
# 1. If --install-dir was provided, use that.
# 2. If running as root (e.g. via sudo), install to /usr/local/bin.
# 3. Otherwise, install to ~/.local/bin (no sudo required).

PREFERRED_ROOT="/usr/local/bin"
FALLBACK_ROOT="${HOME}/.local/bin"

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

INSTALL_BIN="${INSTALL_ROOT}/rq"

if [[ ! -d "${INSTALL_ROOT}" ]]; then
  info "Creating install directory: ${INSTALL_ROOT}"
  mkdir -p "${INSTALL_ROOT}"
fi

info "Copying rq to ${INSTALL_BIN}"
cp "${DOWNLOAD_PATH}" "${INSTALL_BIN}"

if [[ ! -x "${INSTALL_BIN}" ]]; then
  error "Failed to install rq to ${INSTALL_BIN}"
fi

case ":${PATH}:" in
  *:"${INSTALL_ROOT}":*)
    ;;
  *)
    warn "${INSTALL_ROOT} is not in your PATH."
    warn "Add the following line to your shell profile (e.g., ~/.bashrc, ~/.zshrc, or ~/.bash_profile):"
    warn "  export PATH=\"${INSTALL_ROOT}:\$PATH\""
    ;;
esac

rm -f "${DOWNLOAD_PATH}"

printf 'Success: Installed rq to %s\n' "${INSTALL_BIN}" >&2
