# Developer setup script for Windows
# Installs Rust, checks Node.js, and installs npm dependencies

Write-Host "=== Syno Perm Manager - Setup ===" -ForegroundColor Cyan

# Check Node.js
if (!(Get-Command node -ErrorAction SilentlyContinue)) {
    Write-Host "[ERROR] Node.js no encontrado. Instala desde https://nodejs.org/" -ForegroundColor Red
    exit 1
}
Write-Host "[OK] Node.js: $(node --version)" -ForegroundColor Green

# Check Rust
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
if (!(Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "Instalando Rust via rustup..." -ForegroundColor Yellow
    winget install --id Rustlang.Rustup --accept-package-agreements --accept-source-agreements
    $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
    rustup default stable
}
Write-Host "[OK] Rust: $(rustc --version)" -ForegroundColor Green

# Check VS Build Tools
$vsPath = "C:\Program Files (x86)\Microsoft Visual Studio"
if (!(Test-Path $vsPath)) {
    Write-Host "Instalando VS Build Tools con C++..." -ForegroundColor Yellow
    winget install --id Microsoft.VisualStudio.2022.BuildTools --override "--passive --wait --add Microsoft.VisualStudio.Workload.VCTools --add Microsoft.VisualStudio.Component.VC.Tools.x86.x64 --add Microsoft.VisualStudio.Component.Windows11SDK.22621" --accept-package-agreements --accept-source-agreements
}
Write-Host "[OK] VS Build Tools detectado" -ForegroundColor Green

# Install npm dependencies
Write-Host "Instalando dependencias npm..." -ForegroundColor Yellow
npm install
Write-Host "[OK] Dependencias instaladas" -ForegroundColor Green

Write-Host ""
Write-Host "Setup completo! Ejecuta 'npm run tauri dev' para arrancar." -ForegroundColor Cyan
