<#
  build-windows-signed.ps1: builds the Windows installers with signed update packages on this PC.
  Only for when GitHub Actions can't run; normal releases come from .github/workflows/release.yml.

  Run from the repo root, with Optimusic closed:
    powershell -ExecutionPolicy Bypass -File .\scripts\build-windows-signed.ps1
  It asks for the signing-key password without showing it, and keeps it only for this build.
#>
param([string]$Key = "$env:USERPROFILE\.tauri\optimusic.key")

$ErrorActionPreference = "Stop"
if (-not (Test-Path $Key)) { throw "Signing key not found at $Key" }
if (Get-Process optimusic -ErrorAction SilentlyContinue) { throw "Quit Optimusic first (tray, Quit)." }

$secure = Read-Host "Signing key password" -AsSecureString
try {
    $env:TAURI_SIGNING_PRIVATE_KEY = Get-Content -Raw $Key
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = [Net.NetworkCredential]::new("", $secure).Password
    Push-Location "$PSScriptRoot\..\src-tauri"
    cargo tauri build --bundles nsis,msi --config tauri.release.conf.json
    if ($LASTEXITCODE -ne 0) { throw "Build failed (exit $LASTEXITCODE)" }
} finally {
    Pop-Location -ErrorAction SilentlyContinue
    Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY, Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue
}

$bundle = "$PSScriptRoot\..\src-tauri\target\release\bundle"
Get-ChildItem "$bundle\nsis\*.exe*", "$bundle\msi\*.msi*" | Select-Object Name, Length, LastWriteTime | Format-Table -AutoSize
Write-Host "Done. Each installer should have a matching .sig file above." -ForegroundColor Green
