#!/usr/bin/env bash
set -euo pipefail

CONTAINER_NAME="rq-keycloak-uat"
docker stop "$CONTAINER_NAME" >/dev/null 2>&1 || true
echo "Keycloak stopped ($CONTAINER_NAME)"
