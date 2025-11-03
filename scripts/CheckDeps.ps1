# scripts/CheckDeps.ps1
<#
  CheckDeps.ps1 — verify (and optionally install) local dev dependencies for SketchUploader.

  Usage:
    .\scripts\CheckDeps.ps1           # check only (default)
    .\scripts\CheckDeps.ps1 --install # install missing deps (winget required)

  What it checks:
    - Node.js (and npm)
    - Rust (rustup/cargo)
    - MSVC Build Tools + Windows SDK (Windows only)
    - WebView2 Runtime (Windows only)
#>

param(
  [switch]$Install,
  [switch]$Help
)

if ($Help) {
  Write-Host "CheckDeps.ps1 — verify (and optionally install) local dev dependencies for SketchUploader.`n" -ForegroundColor Cyan
  Write-Host "Usage:`n  .\scripts\CheckDeps.ps1           # check only (default)`n  .\scripts\CheckDeps.ps1 --install # install missing deps (winget required)`n"
  exit 0
}

$ErrorActionPreference = "Stop"

function Have([string]$cmd) { return [bool](Get-Command $cmd -ErrorAction SilentlyContinue) }
function HaveWinget { return [bool](Get-Command winget -ErrorAction SilentlyContinue) }

Write-Host "Checking development prerequisites..." -ForegroundColor Cyan

# Basic CLI tools
$haveNode = Have "node"
$haveNpm = Have "npm"
$haveRustup = Have "rustup"
$haveCargo = Have "cargo"

# --- Windows-specific checks ---
$haveMSVC = $false
$haveWinSDK = $false
$haveWV2 = $false
$winget = HaveWinget

if ($IsWindows) {
  # Prefer vswhere if available
  $vswhere = "$env:ProgramFiles(x86)\Microsoft Visual Studio\Installer\vswhere.exe"
  if (Test-Path $vswhere) {
    try {
      $vs = & $vswhere -latest -prerelease -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2>$null
      if ($vs) { $haveMSVC = $true }
    }
    catch { }
  }

  # Fallback: check common registry roots for VS 2022/2019 presence
  if (-not $haveMSVC) {
    $haveMSVC = (Test-Path "HKLM:\SOFTWARE\Microsoft\VisualStudio\17.0") -or
    (Test-Path "HKLM:\SOFTWARE\WOW6432Node\Microsoft\VisualStudio\17.0") -or
    (Test-Path "HKLM:\SOFTWARE\Microsoft\VisualStudio\16.0") -or
    (Test-Path "HKLM:\SOFTWARE\WOW6432Node\Microsoft\VisualStudio\16.0")
  }

  # Windows SDK
  $haveWinSDK = (Test-Path "HKLM:\SOFTWARE\Microsoft\Windows Kits\Installed Roots") -or
  (Test-Path "HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows Kits\Installed Roots")

  # WebView2 Runtime — look for installed binaries
  try {
    $wv2Paths = @(Get-ChildItem -Path "C:\Program Files (x86)\Microsoft\EdgeWebView\Application" -Directory -ErrorAction SilentlyContinue |
      Get-ChildItem -Filter "msedgewebview2.exe" -Recurse -ErrorAction SilentlyContinue)
    $haveWV2 = ($wv2Paths.Count -gt 0)
  }
  catch { $haveWV2 = $false }
}

# --- Report ---
function status($name, $ok) {
  if ($ok) { Write-Host ("  ✔ {0}" -f $name) -ForegroundColor Green }
  else { Write-Host ("  ✗ {0}" -f $name) -ForegroundColor Yellow }
}

status "Node.js"          $haveNode
status "npm"              $haveNpm
status "Rust (rustup)"    $haveRustup
status "Cargo"            $haveCargo
if ($IsWindows) {
  status "MSVC Build Tools" $haveMSVC
  status "Windows SDK"      $haveWinSDK
  status "WebView2 Runtime" $haveWV2
}

# --- Install if requested ---
if ($Install) {
  if (-not $winget) {
    Write-Host "`n--install requested but 'winget' is not available. Install winget or run manually." -ForegroundColor Red
    exit 1
  }

  if (-not $haveNode) {
    Write-Host "Installing Node.js LTS via winget..."
    winget install -e --id OpenJS.NodeJS.LTS --silent --accept-package-agreements --accept-source-agreements
  }
  if (-not $haveRustup) {
    Write-Host "Installing Rust (rustup) via winget..."
    winget install -e --id Rustlang.Rustup --silent --accept-package-agreements --accept-source-agreements
  }
  if ($IsWindows -and -not $haveMSVC) {
    Write-Host "Installing MSVC Build Tools (this may take a while)..."
    winget install -e --id Microsoft.VisualStudio.2022.BuildTools --silent --accept-package-agreements --accept-source-agreements
  }
  if ($IsWindows -and -not $haveWinSDK) {
    Write-Host "Installing Windows 11 SDK (latest)..."
    winget install -e --id Microsoft.WindowsSDK --silent --accept-package-agreements --accept-source-agreements
  }
  if ($IsWindows -and -not $haveWV2) {
    Write-Host "Installing WebView2 Runtime..."
    winget install -e --id Microsoft.EdgeWebView2Runtime --silent --accept-package-agreements --accept-source-agreements
  }
}

Write-Host "`nIf you just installed tools, restart PowerShell/VS Code so PATH updates apply." -ForegroundColor Green
Write-Host "Then run:"
Write-Host "  npm install"
Write-Host "  npm run tauri dev"
