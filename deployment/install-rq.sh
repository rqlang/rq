#!/usr/bin/env bash

set -euo pipefail

OWNER="rqlang"
REPO="rq"
RELEASES_URL="https://github.com/${OWNER}/${REPO}/releases"

RELEASE_TAG=""
INSTALL_DIR=""

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
  -r, --release-tag TAG   GitHub release tag to install (e.g. 0.8.0).
                          If omitted, the latest release is used.
  -d, --install-dir DIR   Directory to install the rq binary into.
                          Overrides the default location logic.
  -h, --help              Show this help and exit.
EOF
}

resolve_latest_tag() {
  local effective_url

  if ! effective_url="$(curl -fsSLI -o /dev/null -w '%{url_effective}' \
    -H "User-Agent: rq-installer-script" \
    "${RELEASES_URL}/latest")"; then
    error "Failed to resolve the latest release from ${RELEASES_URL}/latest."
  fi

  case "${effective_url}" in
    "${RELEASES_URL}/tag/"*)
      printf '%s' "${effective_url#"${RELEASES_URL}/tag/"}"
      ;;
    *)
      error "Could not determine the latest release tag from ${effective_url}."
      ;;
  esac
}

download_release_file() {
  local name="$1"
  local destination="$2"
  local status

  if ! status="$(curl -sSL -o "${destination}" -w '%{http_code}' \
    -H "User-Agent: rq-installer-script" \
    "${RELEASES_URL}/download/${RELEASE_TAG}/${name}")"; then
    error "Failed to download '${name}' from release '${RELEASE_TAG}'."
  fi

  case "${status}" in
    200)
      ;;
    404)
      return 1
      ;;
    *)
      error "Downloading '${name}' from release '${RELEASE_TAG}' returned HTTP ${status}."
      ;;
  esac
}

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{ print $1 }'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{ print $1 }'
  else
    error "Neither sha256sum nor shasum is available to verify the download."
  fi
}

verify_checksum() {
  local expected
  local actual

  if ! download_release_file "SHA256SUMS" "${CHECKSUMS_PATH}"; then
    error "Release '${RELEASE_TAG}' publishes no SHA256SUMS, so '${ASSET_NAME}' cannot be verified and was not installed. Releases before 0.8.0 have none; download them manually from ${RELEASES_URL}."
  fi

  expected="$(awk -v asset="${ASSET_NAME}" '$2 == asset || $2 == "*" asset { print $1 }' "${CHECKSUMS_PATH}")"
  if [[ ! "${expected}" =~ ^[0-9a-f]{64}$ ]]; then
    error "SHA256SUMS of release '${RELEASE_TAG}' has a missing or invalid sha256 for '${ASSET_NAME}'."
  fi

  actual="$(sha256_of "${DOWNLOAD_PATH}")"
  if [[ "${actual}" != "${expected}" ]]; then
    error "Checksum mismatch for '${ASSET_NAME}': expected ${expected}, got ${actual}."
  fi

  info "Checksum verified (sha256 ${actual})."
}

main() {
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

  command -v curl >/dev/null 2>&1 || error "curl is required to download rq. Install it and run this script again."

  OS_NAME="$(uname -s)"
  ARCH="$(uname -m)"
  ASSET_NAME=""

  case "${OS_NAME}" in
    Linux)
      case "${ARCH}" in
        x86_64|amd64)
          ASSET_NAME="rq-linux-x86_64"
          ;;
        aarch64|arm64)
          ASSET_NAME="rq-linux-aarch64"
          ;;
        *)
          error "Unsupported Linux architecture: ${ARCH}. Prebuilt binaries exist for x86_64 and aarch64 only."
          ;;
      esac
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
    mkdir -p "${INSTALL_ROOT}" 2>/dev/null \
      || error "Cannot create ${INSTALL_ROOT}. Run the script with sudo, or pick a directory you own with --install-dir."
  fi

  if [[ ! -w "${INSTALL_ROOT}" ]]; then
    error "Cannot write to ${INSTALL_ROOT}. Run the script with sudo, or pick a directory you own with --install-dir."
  fi

  if [[ -z "${RELEASE_TAG}" ]]; then
    info "Release tag not provided. Resolving the latest release of ${OWNER}/${REPO}."
    RELEASE_TAG="$(resolve_latest_tag)"
  else
    info "Using specified release tag '${RELEASE_TAG}'."
  fi

  info "Installing release '${RELEASE_TAG}' from repository ${OWNER}/${REPO}."

  WORK_DIR="$(mktemp -d)"
  trap 'rm -rf "${WORK_DIR}"' EXIT
  DOWNLOAD_PATH="${WORK_DIR}/rq"
  CHECKSUMS_PATH="${WORK_DIR}/SHA256SUMS"

  info "Downloading '${ASSET_NAME}'."
  if ! download_release_file "${ASSET_NAME}" "${DOWNLOAD_PATH}"; then
    error "Release '${RELEASE_TAG}' has no asset '${ASSET_NAME}', or the release does not exist. See ${RELEASES_URL} for the available releases and assets."
  fi

  verify_checksum

  info "Copying rq to ${INSTALL_BIN}"
  STAGED_BIN="$(mktemp "${INSTALL_ROOT}/.rq.XXXXXX")"
  trap 'rm -rf "${WORK_DIR}" "${STAGED_BIN}"' EXIT
  install -m 755 "${DOWNLOAD_PATH}" "${STAGED_BIN}"

  if ! VERSION_OUTPUT="$("${STAGED_BIN}" --version 2>&1)"; then
    error "The downloaded '${ASSET_NAME}' does not run on this system, so nothing was installed. On Linux it needs glibc 2.34 or newer and OpenSSL 3 (libssl.so.3). Output: ${VERSION_OUTPUT}"
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

  printf 'Success: Installed rq to %s\n' "${INSTALL_BIN}" >&2
}

main "$@"
