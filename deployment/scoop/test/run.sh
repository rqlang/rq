#!/usr/bin/env bash

set -euo pipefail

cd "$(dirname "$0")"
RENDER=../render-manifest.sh
SCRATCH=$(mktemp -d)
trap 'rm -rf "${SCRATCH}"' EXIT
FAILED=0

pass() { printf 'ok   %s\n' "$1"; }
fail() { printf 'FAIL %s\n' "$1"; FAILED=1; }

test_renders_expected_manifest() {
  if "${RENDER}" 1.2.3 SHA256SUMS | diff -u expected.json -; then
    pass "renders expected manifest"
  else
    fail "renders expected manifest"
  fi
}

test_fails_when_asset_missing() {
  grep -v ' rq-windows-x86_64.exe$' SHA256SUMS > "${SCRATCH}/SHA256SUMS"
  if "${RENDER}" 1.2.3 "${SCRATCH}/SHA256SUMS" > /dev/null 2>&1; then
    fail "fails when the asset is missing"
  else
    pass "fails when the asset is missing"
  fi
}

test_fails_on_prerelease_version() {
  if "${RENDER}" 1.2.3-rc.1 SHA256SUMS > /dev/null 2>&1; then
    fail "fails on prerelease version"
  else
    pass "fails on prerelease version"
  fi
}

test_keeps_v_prefixed_tag_in_url_only() {
  if "${RENDER}" v1.2.3 SHA256SUMS | diff -u <(sed 's|/download/1.2.3/|/download/v1.2.3/|' expected.json) -; then
    pass "keeps v-prefixed tag in url only"
  else
    fail "keeps v-prefixed tag in url only"
  fi
}

test_renders_expected_manifest
test_fails_when_asset_missing
test_fails_on_prerelease_version
test_keeps_v_prefixed_tag_in_url_only

exit "${FAILED}"
