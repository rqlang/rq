#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 2 ]]; then
  printf 'Usage: %s TAG SHA256SUMS\n' "$0" >&2
  exit 2
fi

TAG="$1"
VERSION="${TAG#v}"
SUMS_FILE="$2"
BASE_URL="https://github.com/rqlang/rq/releases/download/${TAG}"

[[ "${VERSION}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { printf 'Invalid version: %s\n' "${VERSION}" >&2; exit 1; }
[[ -r "${SUMS_FILE}" ]] || { printf 'Cannot read %s\n' "${SUMS_FILE}" >&2; exit 1; }

sha_of() {
  local sha
  sha=$(awk -v asset="$1" '$2 == asset || $2 == "*" asset { print $1 }' "${SUMS_FILE}")
  [[ "${sha}" =~ ^[0-9a-f]{64}$ ]] || { printf 'Missing or invalid sha256 for %s\n' "$1" >&2; exit 1; }
  printf '%s' "${sha}"
}

WINDOWS_INTEL_SHA=$(sha_of rq-windows-x86_64.exe)

jq -n \
  --arg version "${VERSION}" \
  --arg url "${BASE_URL}/rq-windows-x86_64.exe#/rq.exe" \
  --arg hash "${WINDOWS_INTEL_SHA}" \
  '{
    version: $version,
    description: "Manage and execute HTTP requests defined in .rq files",
    homepage: "https://github.com/rqlang/rq",
    license: "Apache-2.0",
    architecture: {
      "64bit": {
        url: $url,
        hash: $hash
      }
    },
    bin: "rq.exe"
  }'
