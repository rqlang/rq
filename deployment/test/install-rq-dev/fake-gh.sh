#!/usr/bin/env bash

set -euo pipefail

if [[ -n "${FAKE_GH_LOG:-}" ]]; then
  printf '%s\n' "$*" >> "${FAKE_GH_LOG}"
fi

case "$1 ${2:-}" in
  "api "*)
    if [[ -n "${FAKE_GH_API_ERROR:-}" ]]; then
      printf '%s\n' "${FAKE_GH_API_ERROR}" >&2
      exit 1
    fi
    if [[ -f "${FAKE_ARTIFACTS}/run_id" ]]; then
      cat "${FAKE_ARTIFACTS}/run_id"
    fi
    ;;
  "run download")
    name=""
    dir=""
    shift 2
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --name) name="$2"; shift 2 ;;
        --dir) dir="$2"; shift 2 ;;
        -R) shift 2 ;;
        *) shift ;;
      esac
    done
    if [[ ! -d "${FAKE_ARTIFACTS}/${name}" ]]; then
      printf 'no artifact matches any of the names or patterns provided\n' >&2
      exit 1
    fi
    cp -R "${FAKE_ARTIFACTS}/${name}/." "${dir}"
    ;;
  *)
    printf 'fake gh: unexpected command %s\n' "$*" >&2
    exit 2
    ;;
esac
