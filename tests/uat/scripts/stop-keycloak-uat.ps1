#Requires -Version 5.1

$ContainerName = "rq-keycloak-uat"
docker stop $ContainerName 2>$null | Out-Null
Write-Host "Keycloak stopped ($ContainerName)"
