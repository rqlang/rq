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

run_installer_without_install_dir() {
  local path_prefix="$1"
  mkdir -p "${SCRATCH}/home"
  HOME="${SCRATCH}/home" PATH="${path_prefix}${SCRATCH}/bin:${PATH}" FAKE_RELEASES="${SCRATCH}/releases" FAKE_OS=Linux FAKE_ARCH=x86_64 \
    bash "${INSTALLER}" > "${SCRATCH}/output" 2>&1
}

skip() { printf 'skip %s\n' "$1"; }

running_as_root() {
  [[ "$(id -u)" -eq 0 ]]
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

test_installs_to_local_bin_when_not_root() {
  if running_as_root; then
    skip "installs to ~/.local/bin when not root (running as root)"
    return
  fi
  reset_fixture
  add_asset 1.2.3 rq-linux-x86_64 "rq 1.2.3"
  publish_checksums 1.2.3
  set_latest 1.2.3
  if run_installer_without_install_dir "" && [[ "$("${SCRATCH}/home/.local/bin/rq")" == "rq 1.2.3" ]]; then
    pass "installs to ~/.local/bin when not root"
  else
    fail "installs to ~/.local/bin when not root"
  fi
}

test_installs_to_usr_local_bin_when_root() {
  if ! running_as_root; then
    skip "installs to /usr/local/bin when root (not running as root)"
    return
  fi
  if [[ -e /usr/local/bin/rq ]]; then
    skip "installs to /usr/local/bin when root (/usr/local/bin/rq already exists)"
    return
  fi
  reset_fixture
  add_asset 1.2.3 rq-linux-x86_64 "rq 1.2.3"
  publish_checksums 1.2.3
  set_latest 1.2.3
  if run_installer_without_install_dir "" && [[ "$(/usr/local/bin/rq)" == "rq 1.2.3" ]]; then
    pass "installs to /usr/local/bin when root"
  else
    fail "installs to /usr/local/bin when root"
  fi
  rm -f /usr/local/bin/rq
}

test_warns_when_install_dir_not_in_path() {
  reset_fixture
  add_asset 1.2.3 rq-linux-x86_64 "rq 1.2.3"
  publish_checksums 1.2.3
  set_latest 1.2.3
  if run_installer Linux x86_64 && grep -q "${SCRATCH}/install is not in your PATH" "${SCRATCH}/output"; then
    pass "warns when the install directory is not in PATH"
  else
    fail "warns when the install directory is not in PATH"
  fi
}

test_does_not_warn_when_install_dir_in_path() {
  reset_fixture
  add_asset 1.2.3 rq-linux-x86_64 "rq 1.2.3"
  publish_checksums 1.2.3
  set_latest 1.2.3
  if PATH="${SCRATCH}/install:${PATH}" run_installer Linux x86_64 && ! grep -q "not in your PATH" "${SCRATCH}/output"; then
    pass "does not warn when the install directory is in PATH"
  else
    fail "does not warn when the install directory is in PATH"
  fi
}

test_fails_when_latest_release_cannot_be_resolved() {
  reset_fixture
  if ! run_installer Linux x86_64 && grep -q "Could not determine the latest release tag" "${SCRATCH}/output" && [[ -z "$(ls -A "${SCRATCH}/install")" ]]; then
    pass "fails when the latest release cannot be resolved"
  else
    fail "fails when the latest release cannot be resolved"
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
  if ! run_installer Linux x86_64 && grep -q "does not run on this system" "${SCRATCH}/output" && [[ -z "$(ls -A "${SCRATCH}/install")" ]]; then
    pass "fails without leaving files when the downloaded binary does not run"
  else
    fail "fails without leaving files when the downloaded binary does not run"
  fi
}

test_fails_when_install_dir_not_writable() {
  if running_as_root; then
    skip "fails with a clear message when the install directory is not writable (running as root)"
    return
  fi
  reset_fixture
  add_asset 1.2.3 rq-linux-x86_64 "rq 1.2.3"
  publish_checksums 1.2.3
  set_latest 1.2.3
  chmod 555 "${SCRATCH}/install"
  if ! run_installer Linux x86_64 && grep -q "Cannot write to ${SCRATCH}/install" "${SCRATCH}/output"; then
    pass "fails with a clear message when the install directory is not writable"
  else
    fail "fails with a clear message when the install directory is not writable"
  fi
  chmod 755 "${SCRATCH}/install"
}

test_fails_when_install_dir_cannot_be_created() {
  if running_as_root; then
    skip "fails with a clear message when the install directory cannot be created (running as root)"
    return
  fi
  reset_fixture
  add_asset 1.2.3 rq-linux-x86_64 "rq 1.2.3"
  publish_checksums 1.2.3
  set_latest 1.2.3
  chmod 555 "${SCRATCH}/install"
  if ! run_installer Linux x86_64 -d "${SCRATCH}/install/nested" && grep -q "Cannot create ${SCRATCH}/install/nested" "${SCRATCH}/output"; then
    pass "fails with a clear message when the install directory cannot be created"
  else
    fail "fails with a clear message when the install directory cannot be created"
  fi
  chmod 755 "${SCRATCH}/install"
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
test_installs_to_local_bin_when_not_root
test_installs_to_usr_local_bin_when_root
test_warns_when_install_dir_not_in_path
test_does_not_warn_when_install_dir_in_path
test_fails_when_latest_release_cannot_be_resolved
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
test_fails_when_install_dir_not_writable
test_fails_when_install_dir_cannot_be_created
test_replaces_existing_binary

exit "${FAILED}"
