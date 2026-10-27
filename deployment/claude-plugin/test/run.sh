#!/usr/bin/env bash

set -euo pipefail

cd "$(dirname "$0")"
RENDER="$(pwd)/../render-plugin.sh"
PLUGIN="$(cd ../../../src/claude-plugin && pwd)"
SCRATCH=$(mktemp -d)
trap 'rm -rf "${SCRATCH}"' EXIT
FAILED=0

pass() { printf 'ok   %s\n' "$1"; }
fail() { printf 'FAIL %s\n' "$1"; FAILED=1; }

render() {
  rm -rf "${SCRATCH}/out"
  "${RENDER}" "$1" "${2:-${PLUGIN}}" "${SCRATCH}/out"
}

source_copy() {
  rm -rf "${SCRATCH}/source"
  cp -R "${PLUGIN}" "${SCRATCH}/source"
}

test_sets_manifest_version() {
  render 1.3.0 > /dev/null
  if grep -qFx '  "version": "1.3.0",' "${SCRATCH}/out/.claude-plugin/plugin.json"; then
    pass "sets the manifest version"
  else
    fail "sets the manifest version"
  fi
}

test_pins_server_version() {
  render 1.3.0 > /dev/null
  if grep -qF '"@rqlang/mcp@1.3.0"' "${SCRATCH}/out/.mcp.json" && ! grep -qF '0.0.0' "${SCRATCH}/out/.mcp.json"; then
    pass "pins the server version"
  else
    fail "pins the server version"
  fi
}

test_copies_everything_else_unchanged() {
  render 1.3.0 > /dev/null
  if diff -r -x plugin.json -x .mcp.json "${PLUGIN}" "${SCRATCH}/out" > /dev/null \
    && diff <(sed 's/0\.0\.0/1.3.0/' "${PLUGIN}/.claude-plugin/plugin.json") "${SCRATCH}/out/.claude-plugin/plugin.json" > /dev/null \
    && diff <(sed 's/0\.0\.0/1.3.0/' "${PLUGIN}/.mcp.json") "${SCRATCH}/out/.mcp.json" > /dev/null; then
    pass "copies everything else unchanged"
  else
    fail "copies everything else unchanged"
  fi
}

test_includes_marketplace() {
  render 1.3.0 > /dev/null
  if [[ -f "${SCRATCH}/out/.claude-plugin/marketplace.json" ]]; then
    pass "includes the marketplace"
  else
    fail "includes the marketplace"
  fi
}

test_leaves_source_untouched() {
  local before
  before=$(cat "${PLUGIN}/.claude-plugin/plugin.json" "${PLUGIN}/.mcp.json")
  render 1.3.0 > /dev/null
  if [[ "$(cat "${PLUGIN}/.claude-plugin/plugin.json" "${PLUGIN}/.mcp.json")" == "${before}" ]]; then
    pass "leaves the source untouched"
  else
    fail "leaves the source untouched"
  fi
}

test_strips_v_prefix() {
  if [[ $(render v1.3.0) == "1.3.0" ]] && grep -qF '"@rqlang/mcp@1.3.0"' "${SCRATCH}/out/.mcp.json"; then
    pass "strips the v prefix"
  else
    fail "strips the v prefix"
  fi
}

test_fails_on_prerelease_version() {
  if render 1.3.0-rc.1 > /dev/null 2>&1; then
    fail "fails on prerelease version"
  else
    pass "fails on prerelease version"
  fi
}

test_fails_when_out_exists() {
  mkdir -p "${SCRATCH}/out"
  if "${RENDER}" 1.3.0 "${PLUGIN}" "${SCRATCH}/out" > /dev/null 2>&1; then
    fail "fails when the output exists"
  else
    pass "fails when the output exists"
  fi
}

test_fails_when_source_pins_a_release() {
  source_copy
  sed -i.bak 's/@0\.0\.0/@1.2.3/' "${SCRATCH}/source/.mcp.json"
  if render 1.3.0 "${SCRATCH}/source" > /dev/null 2>&1; then
    fail "fails when the source pins a release"
  else
    pass "fails when the source pins a release"
  fi
}

test_sets_manifest_version
test_pins_server_version
test_copies_everything_else_unchanged
test_includes_marketplace
test_leaves_source_untouched
test_strips_v_prefix
test_fails_on_prerelease_version
test_fails_when_out_exists
test_fails_when_source_pins_a_release

exit "${FAILED}"
