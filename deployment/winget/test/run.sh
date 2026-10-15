#!/usr/bin/env bash

set -euo pipefail

cd "$(dirname "$0")"
RENDER=../render-manifests.sh
SCRATCH=$(mktemp -d)
trap 'rm -rf "${SCRATCH}"' EXIT
FAILED=0

pass() { printf 'ok   %s\n' "$1"; }
fail() { printf 'FAIL %s\n' "$1"; FAILED=1; }

render_into() {
  rm -rf "$1"
  mkdir -p "$1"
  "${RENDER}" "$2" "$3" "$1"
}

test_renders_expected_manifests() {
  if render_into "${SCRATCH}/out" 1.2.3 SHA256SUMS && diff -ru expected "${SCRATCH}/out"; then
    pass "renders expected manifests"
  else
    fail "renders expected manifests"
  fi
}

test_fails_when_asset_missing() {
  grep -v ' rq-windows-x86_64.exe$' SHA256SUMS > "${SCRATCH}/SHA256SUMS"
  if render_into "${SCRATCH}/out" 1.2.3 "${SCRATCH}/SHA256SUMS" > /dev/null 2>&1; then
    fail "fails when the asset is missing"
  else
    pass "fails when the asset is missing"
  fi
}

test_fails_on_prerelease_version() {
  if render_into "${SCRATCH}/out" 1.2.3-rc.1 SHA256SUMS > /dev/null 2>&1; then
    fail "fails on prerelease version"
  else
    pass "fails on prerelease version"
  fi
}

test_fails_when_output_dir_missing() {
  if "${RENDER}" 1.2.3 SHA256SUMS "${SCRATCH}/missing" > /dev/null 2>&1; then
    fail "fails when the output directory is missing"
  else
    pass "fails when the output directory is missing"
  fi
}

test_keeps_v_prefixed_tag_in_urls_only() {
  mkdir -p "${SCRATCH}/expected-v"
  for manifest in expected/*.yaml; do
    sed 's|/1\.2\.3/|/v1.2.3/|; s|/tag/1\.2\.3$|/tag/v1.2.3|' "${manifest}" > "${SCRATCH}/expected-v/$(basename "${manifest}")"
  done
  if render_into "${SCRATCH}/out" v1.2.3 SHA256SUMS && diff -ru "${SCRATCH}/expected-v" "${SCRATCH}/out"; then
    pass "keeps v-prefixed tag in urls only"
  else
    fail "keeps v-prefixed tag in urls only"
  fi
}

test_renders_expected_manifests
test_fails_when_asset_missing
test_fails_on_prerelease_version
test_fails_when_output_dir_missing
test_keeps_v_prefixed_tag_in_urls_only

exit "${FAILED}"
