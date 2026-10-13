#!/usr/bin/env bash

set -euo pipefail

RELEASES_URL="https://github.com/rqlang/rq/releases"
output=""
url=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    -o) output="$2"; shift 2 ;;
    -w|-H) shift 2 ;;
    -*) shift ;;
    *) url="$1"; shift ;;
  esac
done

case "${url}" in
  "${RELEASES_URL}/latest")
    printf '%s/tag/%s' "${RELEASES_URL}" "$(cat "${FAKE_RELEASES}/latest")"
    ;;
  "${RELEASES_URL}/download/"*)
    file="${FAKE_RELEASES}/${url#"${RELEASES_URL}/download/"}"
    if [[ -f "${file}" ]]; then
      cp "${file}" "${output}"
      printf '200'
    else
      printf 'Not Found' > "${output}"
      printf '404'
    fi
    ;;
  *)
    printf 'fake curl: unexpected url %s\n' "${url}" >&2
    exit 6
    ;;
esac
