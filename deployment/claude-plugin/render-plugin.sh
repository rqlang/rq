#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 3 ]]; then
  printf 'Usage: %s TAG PLUGIN_DIR OUT_DIR\n' "$0" >&2
  exit 2
fi

TAG="$1"
VERSION="${TAG#v}"
SOURCE="$2"
OUT="$3"
SOURCE_VERSION=0.0.0

[[ "${VERSION}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { printf 'Invalid version: %s\n' "${VERSION}" >&2; exit 1; }
[[ -r "${SOURCE}/.claude-plugin/plugin.json" ]] || { printf 'Cannot read %s\n' "${SOURCE}/.claude-plugin/plugin.json" >&2; exit 1; }
[[ ! -e "${OUT}" ]] || { printf '%s already exists\n' "${OUT}" >&2; exit 1; }
grep -qFx "  \"version\": \"${SOURCE_VERSION}\"," "${SOURCE}/.claude-plugin/plugin.json" || { printf '%s must have version %s\n' "${SOURCE}/.claude-plugin/plugin.json" "${SOURCE_VERSION}" >&2; exit 1; }
grep -qF "\"@rqlang/mcp@${SOURCE_VERSION}\"" "${SOURCE}/.mcp.json" || { printf '%s must pin @rqlang/mcp@%s\n' "${SOURCE}/.mcp.json" "${SOURCE_VERSION}" >&2; exit 1; }

mkdir -p "${OUT}"
cp -R "${SOURCE}/." "${OUT}/"

replace() {
  local file="$1" pattern="$2" scratch
  scratch=$(mktemp)
  sed -E "${pattern}" "${file}" > "${scratch}"
  cat "${scratch}" > "${file}"
  rm -f "${scratch}"
}

replace "${OUT}/.claude-plugin/plugin.json" "s/^  \"version\": \"${SOURCE_VERSION//./\\.}\",$/  \"version\": \"${VERSION}\",/"
replace "${OUT}/.mcp.json" "s/\"@rqlang\/mcp@${SOURCE_VERSION//./\\.}\"/\"@rqlang\/mcp@${VERSION}\"/"
printf '%s\n' "${VERSION}"
