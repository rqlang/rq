#!/usr/bin/env bash

set -euo pipefail

cd "$(dirname "$0")"
INSTALLER="$(pwd)/../../install-rq-dev.sh"
SCRATCH=$(mktemp -d)
trap 'rm -rf "${SCRATCH}"' EXIT
FAILED=0

pass() { printf 'ok   %s\n' "$1"; }
fail() { printf 'FAIL %s\n' "$1"; FAILED=1; }
skip() { printf 'skip %s\n' "$1"; }

running_as_root() {
  [[ "$(id -u)" -eq 0 ]]
}

reset_fixture() {
  rm -rf "${SCRATCH:?}"/*
  mkdir -p "${SCRATCH}/bin" "${SCRATCH}/artifacts" "${SCRATCH}/install"
  cp fake-gh.sh "${SCRATCH}/bin/gh"
  cp ../install-rq/fake-uname.sh "${SCRATCH}/bin/uname"
  chmod +x "${SCRATCH}/bin/gh" "${SCRATCH}/bin/uname"
  printf '42' > "${SCRATCH}/artifacts/run_id"
}

add_artifact() {
  local name="$1"
  local version_line="$2"
  mkdir -p "${SCRATCH}/artifacts/${name}"
  printf '#!/bin/sh\necho "%s"\n' "${version_line}" > "${SCRATCH}/artifacts/${name}/rq"
  chmod 644 "${SCRATCH}/artifacts/${name}/rq"
}

add_broken_artifact() {
  local name="$1"
  mkdir -p "${SCRATCH}/artifacts/${name}"
  printf '#!/bin/sh\necho "error while loading shared libraries: libssl.so.3" >&2\nexit 127\n' > "${SCRATCH}/artifacts/${name}/rq"
}

run_installer() {
  local os="$1"
  local arch="$2"
  shift 2
  PATH="${SCRATCH}/bin:${PATH}" FAKE_ARTIFACTS="${SCRATCH}/artifacts" FAKE_OS="${os}" FAKE_ARCH="${arch}" \
    bash "${INSTALLER}" -d "${SCRATCH}/install" "$@" > "${SCRATCH}/output" 2>&1
}

installed() {
  "${SCRATCH}/install/$1" 2>/dev/null || true
}

install_dir_is_empty() {
  [[ -z "$(ls -A "${SCRATCH}/install")" ]]
}

test_installs_as_rq_dev_by_default() {
  reset_fixture
  add_artifact rq-linux-x86_64 "rq 0.9.0-dev.3"
  if run_installer Linux x86_64 && [[ "$(installed rq-dev)" == "rq 0.9.0-dev.3" ]] && [[ "$(ls -A "${SCRATCH}/install")" == "rq-dev" ]]; then
    pass "installs as rq-dev by default"
  else
    fail "installs as rq-dev by default"
  fi
}

test_keeps_existing_rq() {
  reset_fixture
  add_artifact rq-linux-x86_64 "rq 0.9.0-dev.3"
  printf '#!/bin/sh\necho "rq 0.8.0"\n' > "${SCRATCH}/install/rq"
  chmod +x "${SCRATCH}/install/rq"
  if run_installer Linux x86_64 && [[ "$(installed rq)" == "rq 0.8.0" ]] && [[ "$(installed rq-dev)" == "rq 0.9.0-dev.3" ]]; then
    pass "leaves an installed rq untouched"
  else
    fail "leaves an installed rq untouched"
  fi
}

test_installs_under_custom_name() {
  reset_fixture
  add_artifact rq-linux-x86_64 "rq 0.9.0-dev.3"
  if run_installer Linux x86_64 --name rq && [[ "$(installed rq)" == "rq 0.9.0-dev.3" ]] && [[ ! -e "${SCRATCH}/install/rq-dev" ]]; then
    pass "installs under the name given by --name"
  else
    fail "installs under the name given by --name"
  fi
}

test_rejects_name_with_slash() {
  reset_fixture
  add_artifact rq-linux-x86_64 "rq 0.9.0-dev.3"
  if ! run_installer Linux x86_64 --name ../rq && grep -q "Invalid binary name '../rq'" "${SCRATCH}/output" && install_dir_is_empty; then
    pass "rejects a name that contains a slash"
  else
    fail "rejects a name that contains a slash"
  fi
}

test_picks_artifact() {
  local os="$1"
  local arch="$2"
  local artifact="$3"
  reset_fixture
  add_artifact "${artifact}" "rq ${artifact}"
  if run_installer "${os}" "${arch}" && [[ "$(installed rq-dev)" == "rq ${artifact}" ]]; then
    pass "picks ${artifact} on ${os} ${arch}"
  else
    fail "picks ${artifact} on ${os} ${arch}"
  fi
}

test_fails_on_unsupported_architecture() {
  reset_fixture
  if ! run_installer Linux riscv64 && grep -q "Unsupported Linux architecture: riscv64" "${SCRATCH}/output" && install_dir_is_empty; then
    pass "fails on an unsupported architecture"
  else
    fail "fails on an unsupported architecture"
  fi
}

test_fails_on_unsupported_os() {
  reset_fixture
  if ! run_installer FreeBSD amd64 && grep -q "Unsupported OS: FreeBSD" "${SCRATCH}/output" && install_dir_is_empty; then
    pass "fails on an unsupported OS"
  else
    fail "fails on an unsupported OS"
  fi
}

test_fails_when_gh_missing() {
  local bash_path
  reset_fixture
  mkdir -p "${SCRATCH}/empty-path"
  bash_path="$(command -v bash)"
  if ! PATH="${SCRATCH}/empty-path" "${bash_path}" "${INSTALLER}" -d "${SCRATCH}/install" > "${SCRATCH}/output" 2>&1 \
    && grep -q "GitHub CLI (gh) not found in PATH" "${SCRATCH}/output" && install_dir_is_empty; then
    pass "fails when gh is missing"
  else
    fail "fails when gh is missing"
  fi
}

test_shows_gh_api_error() {
  reset_fixture
  add_artifact rq-linux-x86_64 "rq 0.9.0-dev.3"
  if ! FAKE_GH_API_ERROR="HTTP 401: Bad credentials" run_installer Linux x86_64 \
    && grep -q "HTTP 401: Bad credentials" "${SCRATCH}/output" \
    && grep -q "Failed to query the runs of release_cd.yaml" "${SCRATCH}/output" && install_dir_is_empty; then
    pass "shows the gh error when the runs query fails"
  else
    fail "shows the gh error when the runs query fails"
  fi
}

test_fails_when_no_successful_run() {
  reset_fixture
  rm "${SCRATCH}/artifacts/run_id"
  if ! run_installer Linux x86_64 && grep -q "No successful runs of release_cd.yaml" "${SCRATCH}/output" && install_dir_is_empty; then
    pass "fails when there is no successful run"
  else
    fail "fails when there is no successful run"
  fi
}

test_downloads_from_latest_run() {
  reset_fixture
  add_artifact rq-linux-x86_64 "rq 0.9.0-dev.3"
  if FAKE_GH_LOG="${SCRATCH}/gh.log" run_installer Linux x86_64 && grep -q "^run download 42 --name rq-linux-x86_64 " "${SCRATCH}/gh.log"; then
    pass "downloads the artifact from the latest successful run"
  else
    fail "downloads the artifact from the latest successful run"
  fi
}

test_fails_when_artifact_missing() {
  reset_fixture
  add_artifact rq-linux-x86_64 "rq 0.9.0-dev.3"
  if ! run_installer Linux aarch64 && grep -q "Failed to download artifact 'rq-linux-aarch64' from run 42" "${SCRATCH}/output" && install_dir_is_empty; then
    pass "fails when the artifact is missing"
  else
    fail "fails when the artifact is missing"
  fi
}

test_fails_when_binary_does_not_run() {
  reset_fixture
  add_broken_artifact rq-linux-x86_64
  if ! run_installer Linux x86_64 && grep -q "does not run on this system" "${SCRATCH}/output" && install_dir_is_empty; then
    pass "fails without leaving files when the downloaded binary does not run"
  else
    fail "fails without leaving files when the downloaded binary does not run"
  fi
}

test_fails_when_install_dir_not_writable() {
  if running_as_root; then
    skip "fails before downloading when the install directory is not writable (running as root)"
    return
  fi
  reset_fixture
  add_artifact rq-linux-x86_64 "rq 0.9.0-dev.3"
  chmod 555 "${SCRATCH}/install"
  if ! FAKE_GH_LOG="${SCRATCH}/gh.log" run_installer Linux x86_64 && grep -q "Cannot write to ${SCRATCH}/install" "${SCRATCH}/output" && [[ ! -e "${SCRATCH}/gh.log" ]]; then
    pass "fails before downloading when the install directory is not writable"
  else
    fail "fails before downloading when the install directory is not writable"
  fi
  chmod 755 "${SCRATCH}/install"
}

test_installs_to_local_bin_when_not_root() {
  if running_as_root; then
    skip "installs to ~/.local/bin when not root (running as root)"
    return
  fi
  reset_fixture
  add_artifact rq-linux-x86_64 "rq 0.9.0-dev.3"
  mkdir -p "${SCRATCH}/home"
  if HOME="${SCRATCH}/home" PATH="${SCRATCH}/bin:${PATH}" FAKE_ARTIFACTS="${SCRATCH}/artifacts" FAKE_OS=Linux FAKE_ARCH=x86_64 \
    bash "${INSTALLER}" > "${SCRATCH}/output" 2>&1 \
    && [[ "$("${SCRATCH}/home/.local/bin/rq-dev")" == "rq 0.9.0-dev.3" ]]; then
    pass "installs to ~/.local/bin when not root"
  else
    fail "installs to ~/.local/bin when not root"
  fi
}

test_warns_when_install_dir_not_in_path() {
  reset_fixture
  add_artifact rq-linux-x86_64 "rq 0.9.0-dev.3"
  if run_installer Linux x86_64 && grep -q "${SCRATCH}/install is not in your PATH" "${SCRATCH}/output"; then
    pass "warns when the install directory is not in PATH"
  else
    fail "warns when the install directory is not in PATH"
  fi
}

test_replaces_existing_binary() {
  reset_fixture
  add_artifact rq-linux-x86_64 "rq 0.9.0-dev.3"
  printf '#!/bin/sh\necho "rq 0.9.0-dev.1"\n' > "${SCRATCH}/install/rq-dev"
  chmod +x "${SCRATCH}/install/rq-dev"
  if run_installer Linux x86_64 && [[ "$(installed rq-dev)" == "rq 0.9.0-dev.3" ]] && [[ "$(ls -A "${SCRATCH}/install")" == "rq-dev" ]]; then
    pass "replaces an existing rq-dev without leaving staged files"
  else
    fail "replaces an existing rq-dev without leaving staged files"
  fi
}

test_truncated_script_does_nothing() {
  local size
  local cut
  local cuts=()
  local executed=0
  local main_offset
  reset_fixture
  add_artifact rq-linux-x86_64 "rq 0.9.0-dev.3"
  size=$(wc -c < "${INSTALLER}")
  main_offset="$(grep -bxF 'main "$@"' "${INSTALLER}" | cut -d: -f1 || true)"
  if [[ -z "${main_offset}" ]]; then
    fail "a truncated script runs nothing (no main \"\$@\" call found)"
    return
  fi
  cuts+=("${main_offset}")
  for cut in $(seq 1 40); do
    cuts+=("$(( size * cut / 41 ))")
  done
  for cut in "${cuts[@]}"; do
    head -c "${cut}" "${INSTALLER}" \
      | PATH="${SCRATCH}/bin:${PATH}" FAKE_ARTIFACTS="${SCRATCH}/artifacts" FAKE_GH_LOG="${SCRATCH}/gh.log" FAKE_OS=Linux FAKE_ARCH=x86_64 \
        bash -s -- -d "${SCRATCH}/install" > "${SCRATCH}/output" 2>&1 || true
    if grep -q '^\[' "${SCRATCH}/output"; then
      executed=$((executed + 1))
    fi
  done
  if [[ "${executed}" -eq 0 ]] && [[ ! -e "${SCRATCH}/gh.log" ]] && install_dir_is_empty; then
    pass "a truncated script runs nothing"
  else
    fail "a truncated script runs nothing"
  fi
}

test_installs_as_rq_dev_by_default
test_keeps_existing_rq
test_installs_under_custom_name
test_rejects_name_with_slash
test_picks_artifact Linux x86_64 rq-linux-x86_64
test_picks_artifact Linux aarch64 rq-linux-aarch64
test_picks_artifact Darwin arm64 rq-macos-aarch64
test_picks_artifact Darwin x86_64 rq-macos-x86_64
test_fails_on_unsupported_architecture
test_fails_on_unsupported_os
test_fails_when_gh_missing
test_shows_gh_api_error
test_fails_when_no_successful_run
test_downloads_from_latest_run
test_fails_when_artifact_missing
test_fails_when_binary_does_not_run
test_fails_when_install_dir_not_writable
test_installs_to_local_bin_when_not_root
test_warns_when_install_dir_not_in_path
test_replaces_existing_binary
test_truncated_script_does_nothing

exit "${FAILED}"
