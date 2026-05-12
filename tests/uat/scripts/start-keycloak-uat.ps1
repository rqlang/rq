#Requires -Version 5.1
[CmdletBinding()]
param(
    [string]$KeycloakImage,
    [int]$HostPort
)

$ErrorActionPreference = "Stop"

if (-not $KeycloakImage) { $KeycloakImage = if ($env:KEYCLOAK_IMAGE) { $env:KEYCLOAK_IMAGE } else { "quay.io/keycloak/keycloak:latest" } }
if (-not $HostPort)      { $HostPort      = if ($env:KEYCLOAK_PORT)  { [int]$env:KEYCLOAK_PORT  } else { 9090 } }

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ImportDir = Join-Path $ScriptDir "keycloak\uat"
$ContainerName = "rq-keycloak-uat"
$BaseUrl = "http://localhost:$HostPort"

docker rm -f $ContainerName 2>$null | Out-Null

$ImportDirUnix = $ImportDir -replace "\\", "/" -replace "^([A-Za-z]):", { "/$($args[0].Value.ToLower())" }

docker run -d --rm `
    --name $ContainerName `
    -p "${HostPort}:8080" `
    -e KEYCLOAK_ADMIN=admin `
    -e KEYCLOAK_ADMIN_PASSWORD=admin `
    -v "${ImportDirUnix}:/opt/keycloak/data/import:ro" `
    $KeycloakImage `
    start-dev --import-realm

Write-Host "Waiting for Keycloak to be ready..."
$ready = $false
for ($i = 1; $i -le 60; $i++) {
    try {
        $null = Invoke-WebRequest -Uri "$BaseUrl/" -UseBasicParsing -TimeoutSec 2 -ErrorAction Stop
        $ready = $true
        break
    } catch {
        Start-Sleep -Seconds 1
    }
}

if (-not $ready) {
    Write-Error "Keycloak failed to start within 60 seconds"
    exit 1
}

$CertDir = Join-Path $ScriptDir "keycloak\uat\generated"
$CertKey = Join-Path $CertDir "rq-uat-cc-cert.key"
$CertCrt = Join-Path $CertDir "rq-uat-cc-cert.crt"
$CertP12 = Join-Path $CertDir "rq-uat-cc-cert.p12"
$CertPassword = "rq-uat-p12"

if (-not (Test-Path $CertDir)) {
    New-Item -ItemType Directory -Path $CertDir | Out-Null
}

if (-not (Test-Path $CertP12)) {
    Remove-Item $CertKey, $CertCrt, $CertP12 -ErrorAction SilentlyContinue

    $cert = New-SelfSignedCertificate `
        -Subject "CN=rq-uat-cc-cert" `
        -KeyAlgorithm RSA `
        -KeyLength 2048 `
        -CertStoreLocation "Cert:\CurrentUser\My" `
        -NotAfter (Get-Date).AddDays(365) `
        -KeyExportPolicy Exportable

    try {
        $secPassword = ConvertTo-SecureString -String $CertPassword -Force -AsPlainText
        Export-PfxCertificate -Cert $cert -FilePath $CertP12 -Password $secPassword | Out-Null

        $certBytes = $cert.Export([System.Security.Cryptography.X509Certificates.X509ContentType]::Cert)
        [System.IO.File]::WriteAllBytes($CertCrt, $certBytes)

        Write-Host "Certificate generated: $CertP12"
    } finally {
        Remove-Item "Cert:\CurrentUser\My\$($cert.Thumbprint)" -ErrorAction SilentlyContinue
    }
}

$ClientId = "rq-uat-cc-cert"

docker exec $ContainerName /opt/keycloak/bin/kcadm.sh config credentials `
    --server http://localhost:8080 `
    --realm master `
    --user admin `
    --password admin | Out-Null

$existingJson = docker exec $ContainerName /opt/keycloak/bin/kcadm.sh get clients -r rq-uat -q "clientId=$ClientId" --fields id --format csv 2>$null
$existingId = ($existingJson | Select-Object -Last 1) -replace '"', '' -replace '\r', ''

if (-not $existingId) {
    docker exec $ContainerName /opt/keycloak/bin/kcadm.sh create clients `
        -r rq-uat `
        -s "clientId=$ClientId" `
        -s "name=rq-uat (Client Credentials, Signed JWT)" `
        -s enabled=true `
        -s protocol=openid-connect `
        -s publicClient=false `
        -s standardFlowEnabled=false `
        -s implicitFlowEnabled=false `
        -s directAccessGrantsEnabled=false `
        -s serviceAccountsEnabled=true `
        -s clientAuthenticatorType=client-jwt | Out-Null
}

$cidJson = docker exec $ContainerName /opt/keycloak/bin/kcadm.sh get clients -r rq-uat -q "clientId=$ClientId" --fields id --format csv
$cid = ($cidJson | Select-Object -Last 1) -replace '"', '' -replace '\r', ''

$tokenResponse = Invoke-RestMethod -Method Post `
    -Uri "$BaseUrl/realms/master/protocol/openid-connect/token" `
    -ContentType "application/x-www-form-urlencoded" `
    -Body "client_id=admin-cli&username=admin&password=admin&grant_type=password"

$token = $tokenResponse.access_token

$p12Bytes = [System.IO.File]::ReadAllBytes($CertP12)
$boundary = [System.Guid]::NewGuid().ToString()
$CRLF = "`r`n"

$bodyParts = @(
    "--$boundary$CRLF" +
    "Content-Disposition: form-data; name=`"keystoreFormat`"$CRLF$CRLF" +
    "PKCS12",
    "--$boundary$CRLF" +
    "Content-Disposition: form-data; name=`"keyAlias`"$CRLF$CRLF" +
    "rq-uat-cc-cert",
    "--$boundary$CRLF" +
    "Content-Disposition: form-data; name=`"storePassword`"$CRLF$CRLF" +
    $CertPassword,
    "--$boundary$CRLF" +
    "Content-Disposition: form-data; name=`"keyPassword`"$CRLF$CRLF" +
    $CertPassword
)

$encoding = [System.Text.Encoding]::UTF8
$bodyBytes = [System.Collections.Generic.List[byte]]::new()

foreach ($part in $bodyParts) {
    $bodyBytes.AddRange($encoding.GetBytes($part + $CRLF))
}

$fileHeader = "--$boundary$CRLF" +
    "Content-Disposition: form-data; name=`"file`"; filename=`"rq-uat-cc-cert.p12`"$CRLF" +
    "Content-Type: application/octet-stream$CRLF$CRLF"

$bodyBytes.AddRange($encoding.GetBytes($fileHeader))
$bodyBytes.AddRange($p12Bytes)
$bodyBytes.AddRange($encoding.GetBytes("$CRLF--$boundary--$CRLF"))

$uploadUri = "$BaseUrl/admin/realms/rq-uat/clients/$cid/certificates/jwt.credential/upload-certificate"
Invoke-RestMethod -Method Post `
    -Uri $uploadUri `
    -Headers @{ Authorization = "Bearer $token" } `
    -ContentType "multipart/form-data; boundary=$boundary" `
    -Body $bodyBytes.ToArray() | Out-Null

Write-Host @"
Keycloak started: $BaseUrl

Realm: rq-uat
Admin login: admin / admin
Test user: uat / uat

Authorization endpoint:
  $BaseUrl/realms/rq-uat/protocol/openid-connect/auth
Token endpoint:
  $BaseUrl/realms/rq-uat/protocol/openid-connect/token

Clients:
  - rq-uat-ac (public, auth code)
  - rq-uat-implicit (public, implicit)
  - rq-uat-cc (confidential, client credentials) secret: rq-uat-secret
  - rq-uat-cc-cert (confidential, client credentials via Signed JWT)

Signed JWT client credentials (cert) materials:
  - PKCS#12: $CertP12
  - Password: $CertPassword
"@
