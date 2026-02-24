#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
IMPORT_DIR="$SCRIPT_DIR/keycloak/uat"

CONTAINER_NAME="rq-keycloak-uat"
IMAGE="${KEYCLOAK_IMAGE:-quay.io/keycloak/keycloak:latest}"
HOST_PORT="${KEYCLOAK_PORT:-9090}"

docker rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true

docker run -d --rm \
  --name "$CONTAINER_NAME" \
  -p "${HOST_PORT}:8080" \
  -e KEYCLOAK_ADMIN=admin \
  -e KEYCLOAK_ADMIN_PASSWORD=admin \
  -v "$IMPORT_DIR:/opt/keycloak/data/import:ro" \
  "$IMAGE" \
  start-dev --import-realm

BASE_URL="http://localhost:${HOST_PORT}"

for i in $(seq 1 60); do
  if curl -fsS "${BASE_URL}/" >/dev/null 2>&1; then
    break
  fi
  sleep 1
done

CERT_DIR="$SCRIPT_DIR/keycloak/uat/generated"
CERT_KEY="$CERT_DIR/rq-uat-cc-cert.key"
CERT_CRT="$CERT_DIR/rq-uat-cc-cert.crt"
CERT_P12="$CERT_DIR/rq-uat-cc-cert.p12"
CERT_PASSWORD="rq-uat-p12"

if command -v openssl >/dev/null 2>&1; then
  mkdir -p "$CERT_DIR"

  if [[ ! -f "$CERT_P12" ]]; then
    rm -f "$CERT_KEY" "$CERT_CRT" "$CERT_P12"
    openssl req -x509 -newkey rsa:2048 -nodes \
      -keyout "$CERT_KEY" \
      -out "$CERT_CRT" \
      -days 365 \
      -subj "/CN=rq-uat-cc-cert" \
      >/dev/null 2>&1

    openssl pkcs12 -export \
      -out "$CERT_P12" \
      -inkey "$CERT_KEY" \
      -in "$CERT_CRT" \
      -name rq-uat-cc-cert \
      -passout "pass:${CERT_PASSWORD}" \
      >/dev/null 2>&1
  fi

  if command -v python3 >/dev/null 2>&1; then
    docker exec "$CONTAINER_NAME" /opt/keycloak/bin/kcadm.sh config credentials \
      --server http://localhost:8080 \
      --realm master \
      --user admin \
      --password admin \
      >/dev/null

    CLIENT_ID="rq-uat-cc-cert"
    EXISTING_ID=$(docker exec "$CONTAINER_NAME" /opt/keycloak/bin/kcadm.sh get clients -r rq-uat -q clientId="$CLIENT_ID" --fields id --format csv 2>/dev/null | tail -n 1 | tr -d '"' | tr -d '\r')

    if [[ -z "${EXISTING_ID}" ]]; then
      docker exec "$CONTAINER_NAME" /opt/keycloak/bin/kcadm.sh create clients \
        -r rq-uat \
        -s clientId="$CLIENT_ID" \
        -s name="rq-uat (Client Credentials, Signed JWT)" \
        -s enabled=true \
        -s protocol=openid-connect \
        -s publicClient=false \
        -s standardFlowEnabled=false \
        -s implicitFlowEnabled=false \
        -s directAccessGrantsEnabled=false \
        -s serviceAccountsEnabled=true \
        -s clientAuthenticatorType=client-jwt \
        >/dev/null
    fi

    CID=$(docker exec "$CONTAINER_NAME" /opt/keycloak/bin/kcadm.sh get clients -r rq-uat -q clientId="$CLIENT_ID" --fields id --format csv | tail -n 1 | tr -d '"' | tr -d '\r')
    TOKEN=$(curl -fsS -X POST "${BASE_URL}/realms/master/protocol/openid-connect/token" \
      -d client_id=admin-cli \
      -d username=admin \
      -d password=admin \
      -d grant_type=password \
      | python3 -c 'import sys,json; print(json.load(sys.stdin)["access_token"])')

    curl -fsS -X POST "${BASE_URL}/admin/realms/rq-uat/clients/${CID}/certificates/jwt.credential/upload-certificate" \
      -H "Authorization: Bearer ${TOKEN}" \
      -F "keystoreFormat=PKCS12" \
      -F "keyAlias=rq-uat-cc-cert" \
      -F "storePassword=${CERT_PASSWORD}" \
      -F "keyPassword=${CERT_PASSWORD}" \
      -F "file=@${CERT_P12}" \
      >/dev/null
  fi
fi

cat <<EOF
Keycloak started: ${BASE_URL}

Realm: rq-uat
Admin login: admin / admin
Test user: uat / uat

Authorization endpoint:
  ${BASE_URL}/realms/rq-uat/protocol/openid-connect/auth
Token endpoint:
  ${BASE_URL}/realms/rq-uat/protocol/openid-connect/token

Clients:
  - rq-uat-ac (public, auth code)
  - rq-uat-implicit (public, implicit)
  - rq-uat-cc (confidential, client credentials) secret: rq-uat-secret
  - rq-uat-cc-cert (confidential, client credentials via Signed JWT)

Signed JWT client credentials (cert) materials:
  - PKCS#12: ${CERT_P12}
  - Password: ${CERT_PASSWORD}
EOF
