# VS Code Extension UAT rq Fixtures

This folder is a UAT workspace for the VS Code extension.

It contains a curated set of `.rq` files for manual/UAT testing of:

- Request Explorer discovery and grouping
- Environment selection
- Running requests and surfacing parse errors
- Auth provider discovery and OAuth flows (where applicable)

No real credentials are stored here. Provide values through a local `.env` file.

Keycloak:

- Start: [tests/uat/scripts/start-keycloak-uat.sh](tests/uat/scripts/start-keycloak-uat.sh)
- Stop: [tests/uat/scripts/stop-keycloak-uat.sh](tests/uat/scripts/stop-keycloak-uat.sh)
