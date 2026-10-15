#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 3 ]]; then
  printf 'Usage: %s TAG SHA256SUMS OUTPUT_DIR\n' "$0" >&2
  exit 2
fi

TAG="$1"
VERSION="${TAG#v}"
SUMS_FILE="$2"
OUTPUT_DIR="$3"
PACKAGE_IDENTIFIER="rqlang.rq"
MANIFEST_VERSION="1.12.0"
BASE_URL="https://github.com/rqlang/rq/releases/download/${TAG}"

[[ "${VERSION}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { printf 'Invalid version: %s\n' "${VERSION}" >&2; exit 1; }
[[ -r "${SUMS_FILE}" ]] || { printf 'Cannot read %s\n' "${SUMS_FILE}" >&2; exit 1; }
[[ -d "${OUTPUT_DIR}" ]] || { printf 'Not a directory: %s\n' "${OUTPUT_DIR}" >&2; exit 1; }

sha_of() {
  local sha
  sha=$(awk -v asset="$1" '$2 == asset || $2 == "*" asset { print $1 }' "${SUMS_FILE}")
  [[ "${sha}" =~ ^[0-9a-f]{64}$ ]] || { printf 'Missing or invalid sha256 for %s\n' "$1" >&2; exit 1; }
  printf '%s' "${sha}" | tr '[:lower:]' '[:upper:]'
}

WINDOWS_INTEL_SHA=$(sha_of rq-windows-x86_64.exe)

cat > "${OUTPUT_DIR}/${PACKAGE_IDENTIFIER}.yaml" <<MANIFEST
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.version.${MANIFEST_VERSION}.schema.json
PackageIdentifier: ${PACKAGE_IDENTIFIER}
PackageVersion: ${VERSION}
DefaultLocale: en-US
ManifestType: version
ManifestVersion: ${MANIFEST_VERSION}
MANIFEST

cat > "${OUTPUT_DIR}/${PACKAGE_IDENTIFIER}.installer.yaml" <<MANIFEST
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.installer.${MANIFEST_VERSION}.schema.json
PackageIdentifier: ${PACKAGE_IDENTIFIER}
PackageVersion: ${VERSION}
InstallerType: portable
Commands:
- rq
Installers:
- Architecture: x64
  InstallerUrl: ${BASE_URL}/rq-windows-x86_64.exe
  InstallerSha256: ${WINDOWS_INTEL_SHA}
ManifestType: installer
ManifestVersion: ${MANIFEST_VERSION}
MANIFEST

cat > "${OUTPUT_DIR}/${PACKAGE_IDENTIFIER}.locale.en-US.yaml" <<MANIFEST
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.defaultlocale.${MANIFEST_VERSION}.schema.json
PackageIdentifier: ${PACKAGE_IDENTIFIER}
PackageVersion: ${VERSION}
PackageLocale: en-US
Publisher: rqlang
PublisherUrl: https://github.com/rqlang
PublisherSupportUrl: https://github.com/rqlang/rq/issues
PackageName: rq
PackageUrl: https://github.com/rqlang/rq
License: Apache-2.0
LicenseUrl: https://github.com/rqlang/rq/blob/main/LICENSE
ShortDescription: Manage and execute HTTP requests defined in .rq files
Moniker: rqlang
Tags:
- api
- cli
- http
- http-client
- rest
ReleaseNotesUrl: https://github.com/rqlang/rq/releases/tag/${TAG}
ManifestType: defaultLocale
ManifestVersion: ${MANIFEST_VERSION}
MANIFEST
