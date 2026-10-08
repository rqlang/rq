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

MACOS_ARM_SHA=$(sha_of rq-macos-aarch64)
MACOS_INTEL_SHA=$(sha_of rq-macos-x86_64)
LINUX_ARM_SHA=$(sha_of rq-linux-aarch64)
LINUX_INTEL_SHA=$(sha_of rq-linux-x86_64)

cat <<FORMULA
class Rqlang < Formula
  desc "Manage and execute HTTP requests defined in .rq files"
  homepage "https://github.com/rqlang/rq"
  license "Apache-2.0"

  on_macos do
    on_arm do
      url "${BASE_URL}/rq-macos-aarch64"
      sha256 "${MACOS_ARM_SHA}"
    end
    on_intel do
      url "${BASE_URL}/rq-macos-x86_64"
      sha256 "${MACOS_INTEL_SHA}"
    end
  end

  on_linux do
    on_arm do
      url "${BASE_URL}/rq-linux-aarch64"
      sha256 "${LINUX_ARM_SHA}"
    end
    on_intel do
      url "${BASE_URL}/rq-linux-x86_64"
      sha256 "${LINUX_INTEL_SHA}"
    end
  end

  def install
    bin.install Dir["rq-*"].first => "rq"
    chmod 0755, bin/"rq"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/rq --version")
  end
end
FORMULA
