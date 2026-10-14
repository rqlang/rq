#!/usr/bin/env bash

set -euo pipefail

cd "$(dirname "$0")"
INSTALLER="$(pwd)/../../install-rq.sh"
SCRATCH=$(mktemp -d)
trap 'rm -rf "${SCRATCH}"' EXIT
FAILED=0

pass() { printf 'ok   %s\n' "$1"; }
fail() { printf 'FAIL %s\n' "$1"; FAILED=1; }

sha256_line() {
  if command -v sha256sum >/dev/null 2>&1; then
    (cd "$(dirname "$1")" && sha256sum "$(basename "$1")")
  else
    (cd "$(dirname "$1")" && shasum -a 256 "$(basename "$1")")
  fi
}

reset_fixture() {
  rm -rf "${SCRATCH:?}"/*
  mkdir -p "${SCRATCH}/bin" "${SCRATCH}/releases" "${SCRATCH}/install"
  cp fake-curl.sh "${SCRATCH}/bin/curl"
  cp fake-uname.sh "${SCRATCH}/bin/uname"
  chmod +x "${SCRATCH}/bin/curl" "${SCRATCH}/bin/uname"
}

add_asset() {
  local tag="$1"
  local name="$2"
  local version_line="$3"
  mkdir -p "${SCRATCH}/releases/${tag}"
  printf '#!/bin/sh\necho "%s"\n' "${version_line}" > "${SCRATCH}/releases/${tag}/${name}"
}

add_broken_asset() {
  local tag="$1"
  local name="$2"
  mkdir -p "${SCRATCH}/releases/${tag}"
  printf '#!/bin/sh\necho "error while loading shared libraries: libssl.so.3" >&2\nexit 127\n' > "${SCRATCH}/releases/${tag}/${name}"
}

publish_checksums() {
  local tag="$1"
  local file
  : > "${SCRATCH}/releases/${tag}/SHA256SUMS.tmp"
  for file in "${SCRATCH}/releases/${tag}"/rq-*; do
    sha256_line "${file}" >> "${SCRATCH}/releases/${tag}/SHA256SUMS.tmp"
  done
  mv "${SCRATCH}/releases/${tag}/SHA256SUMS.tmp" "${SCRATCH}/releases/${tag}/SHA256SUMS"
}

set_latest() {
  printf '%s' "$1" > "${SCRATCH}/releases/latest"
}

run_installer() {
  local os="$1"
  local arch="$2"
  shift 2
  PATH="${SCRATCH}/bin:${PATH}" FAKE_RELEASES="${SCRATCH}/releases" FAKE_OS="${os}" FAKE_ARCH="${arch}" \
    bash "${INSTALLER}" -d "${SCRATCH}/install" "$@" > "${SCRATCH}/output" 2>&1
}

installed_version() {
  "${SCRATCH}/install/rq" 2>/dev/null || true
}

test_installs_latest_release() {
  reset_fixture
  add_asset 1.2.3 rq-linux-x86_64 "rq 1.2.3"
  publish_checksums 1.2.3
  set_latest 1.2.3
  if run_installer Linux x86_64 && [[ "$(installed_version)" == "rq 1.2.3" ]]; then
    pass "installs the latest release"
  else
    fail "installs the latest release"
  fi
}

test_installs_requested_release() {
  reset_fixture
  add_asset 1.0.0 rq-linux-x86_64 "rq 1.0.0"
  add_asset 1.2.3 rq-linux-x86_64 "rq 1.2.3"
  publish_checksums 1.0.0
  publish_checksums 1.2.3
  set_latest 1.2.3
  if run_installer Linux x86_64 -r 1.0.0 && [[ "$(installed_version)" == "rq 1.0.0" ]]; then
    pass "installs the requested release"
  else
    fail "installs the requested release"
  fi
}

test_picks_aarch64_asset_on_arm_linux() {
  reset_fixture
  add_asset 1.2.3 rq-linux-x86_64 "rq x86_64"
  add_asset 1.2.3 rq-linux-aarch64 "rq aarch64"
  publish_checksums 1.2.3
  set_latest 1.2.3
  if run_installer Linux aarch64 && [[ "$(installed_version)" == "rq aarch64" ]]; then
    pass "picks the aarch64 asset on ARM Linux"
  else
    fail "picks the aarch64 asset on ARM Linux"
  fi
}

test_picks_aarch64_asset_on_apple_silicon() {
  reset_fixture
  add_asset 1.2.3 rq-macos-x86_64 "rq x86_64"
  add_asset 1.2.3 rq-macos-aarch64 "rq aarch64"
  publish_checksums 1.2.3
  set_latest 1.2.3
  if run_installer Darwin arm64 && [[ "$(installed_version)" == "rq aarch64" ]]; then
    pass "picks the aarch64 asset on Apple Silicon"
  else
    fail "picks the aarch64 asset on Apple Silicon"
  fi
}

test_fails_on_unsupported_architecture() {
  reset_fixture
  set_latest 1.2.3
  if ! run_installer Linux i686 && grep -q "Unsupported Linux architecture: i686" "${SCRATCH}/output"; then
    pass "fails on an unsupported architecture"
  else
    fail "fails on an unsupported architecture"
  fi
}

test_fails_when_asset_missing() {
  reset_fixture
  add_asset 1.2.3 rq-linux-x86_64 "rq 1.2.3"
  publish_checksums 1.2.3
  set_latest 1.2.3
  if ! run_installer Linux aarch64 && grep -q "has no asset 'rq-linux-aarch64'" "${SCRATCH}/output" && [[ ! -e "${SCRATCH}/install/rq" ]]; then
    pass "fails when the asset is missing"
  else
    fail "fails when the asset is missing"
  fi
}

test_fails_on_checksum_mismatch() {
  reset_fixture
  add_asset 1.2.3 rq-linux-x86_64 "rq 1.2.3"
  publish_checksums 1.2.3
  add_asset 1.2.3 rq-linux-x86_64 "rq tampered"
  set_latest 1.2.3
  if ! run_installer Linux x86_64 && grep -q "Checksum mismatch" "${SCRATCH}/output" && [[ ! -e "${SCRATCH}/install/rq" ]]; then
    pass "fails on a checksum mismatch"
  else
    fail "fails on a checksum mismatch"
  fi
}

test_fails_when_checksum_entry_missing() {
  reset_fixture
  add_asset 1.2.3 rq-linux-x86_64 "rq 1.2.3"
  publish_checksums 1.2.3
  grep -v ' rq-linux-x86_64$' "${SCRATCH}/releases/1.2.3/SHA256SUMS" > "${SCRATCH}/SHA256SUMS" || true
  mv "${SCRATCH}/SHA256SUMS" "${SCRATCH}/releases/1.2.3/SHA256SUMS"
  set_latest 1.2.3
  if ! run_installer Linux x86_64 && grep -q "missing or invalid sha256 for 'rq-linux-x86_64'" "${SCRATCH}/output" && [[ ! -e "${SCRATCH}/install/rq" ]]; then
    pass "fails when SHA256SUMS has no entry for the asset"
  else
    fail "fails when SHA256SUMS has no entry for the asset"
  fi
}

test_fails_when_checksum_entry_invalid() {
  reset_fixture
  add_asset 1.2.3 rq-linux-x86_64 "rq 1.2.3"
  printf '<html>Not Found</html>  rq-linux-x86_64\n' > "${SCRATCH}/releases/1.2.3/SHA256SUMS"
  set_latest 1.2.3
  if ! run_installer Linux x86_64 && grep -q "missing or invalid sha256 for 'rq-linux-x86_64'" "${SCRATCH}/output" && [[ ! -e "${SCRATCH}/install/rq" ]]; then
    pass "fails when SHA256SUMS has an invalid sha256 for the asset"
  else
    fail "fails when SHA256SUMS has an invalid sha256 for the asset"
  fi
}

test_fails_when_checksum_entry_duplicated() {
  reset_fixture
  add_asset 1.2.3 rq-linux-x86_64 "rq 1.2.3"
  publish_checksums 1.2.3
  cat "${SCRATCH}/releases/1.2.3/SHA256SUMS" "${SCRATCH}/releases/1.2.3/SHA256SUMS" > "${SCRATCH}/SHA256SUMS"
  mv "${SCRATCH}/SHA256SUMS" "${SCRATCH}/releases/1.2.3/SHA256SUMS"
  set_latest 1.2.3
  if ! run_installer Linux x86_64 && grep -q "missing or invalid sha256 for 'rq-linux-x86_64'" "${SCRATCH}/output" && [[ ! -e "${SCRATCH}/install/rq" ]]; then
    pass "fails when SHA256SUMS lists the asset twice"
  else
    fail "fails when SHA256SUMS lists the asset twice"
  fi
}

test_fails_when_release_has_no_checksums() {
  reset_fixture
  add_asset 1.2.3 rq-linux-x86_64 "rq 1.2.3"
  set_latest 1.2.3
  if ! run_installer Linux x86_64 && grep -q "publishes no SHA256SUMS" "${SCRATCH}/output" && [[ ! -e "${SCRATCH}/install/rq" ]]; then
    pass "fails when the release has no SHA256SUMS"
  else
    fail "fails when the release has no SHA256SUMS"
  fi
}

test_fails_when_binary_does_not_run() {
  reset_fixture
  add_broken_asset 1.2.3 rq-linux-x86_64
  publish_checksums 1.2.3
  set_latest 1.2.3
  if ! run_installer Linux x86_64 && grep -q "does not run on this system" "${SCRATCH}/output" && [[ ! -e "${SCRATCH}/install/rq" ]]; then
    pass "fails when the downloaded binary does not run"
  else
    fail "fails when the downloaded binary does not run"
  fi
}

test_replaces_existing_binary() {
  reset_fixture
  add_asset 1.2.3 rq-linux-x86_64 "rq 1.2.3"
  publish_checksums 1.2.3
  set_latest 1.2.3
  printf '#!/bin/sh\necho "rq 0.0.1"\n' > "${SCRATCH}/install/rq"
  chmod +x "${SCRATCH}/install/rq"
  if run_installer Linux x86_64 && [[ "$(installed_version)" == "rq 1.2.3" ]] && [[ "$(ls -A "${SCRATCH}/install")" == "rq" ]]; then
    pass "replaces an existing binary without leaving staged files"
  else
    fail "replaces an existing binary without leaving staged files"
  fi
}

test_installs_latest_release
test_installs_requested_release
test_picks_aarch64_asset_on_arm_linux
test_picks_aarch64_asset_on_apple_silicon
test_fails_on_unsupported_architecture
test_fails_when_asset_missing
test_fails_on_checksum_mismatch
test_fails_when_checksum_entry_missing
test_fails_when_checksum_entry_invalid
test_fails_when_checksum_entry_duplicated
test_fails_when_release_has_no_checksums
test_fails_when_binary_does_not_run
test_replaces_existing_binary

exit "${FAILED}"
