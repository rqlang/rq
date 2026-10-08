#!/usr/bin/env bash

set -euo pipefail

cd "$(dirname "$0")"
RENDER=../render-formula.sh
SCRATCH=$(mktemp -d)
trap 'rm -rf "${SCRATCH}"' EXIT
FAILED=0

pass() { printf 'ok   %s\n' "$1"; }
fail() { printf 'FAIL %s\n' "$1"; FAILED=1; }

test_renders_expected_formula() {
  if "${RENDER}" 1.2.3 SHA256SUMS | diff -u expected.rb -; then
    pass "renders expected formula"
  else
    fail "renders expected formula"
  fi
}

test_rendered_formula_is_valid_ruby() {
  "${RENDER}" 1.2.3 SHA256SUMS > "${SCRATCH}/rqlang.rb"
  if ruby -c "${SCRATCH}/rqlang.rb" > /dev/null; then
    pass "rendered formula is valid ruby"
  else
    fail "rendered formula is valid ruby"
  fi
}

test_fails_when_asset_missing() {
  grep -v ' rq-linux-aarch64$' SHA256SUMS > "${SCRATCH}/SHA256SUMS"
  if "${RENDER}" 1.2.3 "${SCRATCH}/SHA256SUMS" > /dev/null 2>&1; then
    fail "fails when an asset is missing"
  else
    pass "fails when an asset is missing"
  fi
}

test_fails_on_prerelease_version() {
  if "${RENDER}" 1.2.3-rc.1 SHA256SUMS > /dev/null 2>&1; then
    fail "fails on prerelease version"
  else
    pass "fails on prerelease version"
  fi
}

test_fails_on_v_prefixed_version() {
  if "${RENDER}" v1.2.3 SHA256SUMS > /dev/null 2>&1; then
    fail "fails on v-prefixed version"
  else
    pass "fails on v-prefixed version"
  fi
}

test_renders_expected_formula
test_rendered_formula_is_valid_ruby
test_fails_when_asset_missing
test_fails_on_prerelease_version
test_fails_on_v_prefixed_version

exit "${FAILED}"
