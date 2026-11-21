# ==============================================================================
# YAGO Installer & Updater for Windows (PowerShell)
# https://github.com/rexthewise-xd/YAGO
# ==============================================================================
[CmdletBinding()]
param(
    [switch]$Update,
    [switch]$Uninstall,
    [switch]$Portable,
    [string]$InstallPath = "$env:LOCALAPPDATA\Programs\YAGO",
    [string]$LocalSource = ""
)

$ErrorActionPreference = "Stop"
$Repo = "rexthewise-xd/YAGO"
$AppName = "YAGO"

function Write-Info { Write-Host "==> $args" -ForegroundColor Cyan }
function Write-Success { Write-Host "==> $args" -ForegroundColor Green }
function Write-WarningMsg { Write-Host "==> WARNING: $args" -ForegroundColor Yellow }
function Write-ErrorMsg { Write-Host "==> ERROR: $args" -ForegroundColor Red; exit 1 }

# Handle Uninstall
if ($Uninstall) {
    Write-Info "Uninstalling $AppName..."
    $uninstaller = Join-Path $InstallPath "Uninstall.exe"
    if (Test-Path $uninstaller) {
        Start-Process -FilePath $uninstaller -ArgumentList "/S" -Wait
    } else {
        if (Test-Path $InstallPath) {
            Remove-Item -Path $InstallPath -Recurse -Force
        }
        $desktopLnk = Join-Path ([Environment]::GetFolderPath("Desktop")) "$AppName.lnk"
        if (Test-Path $desktopLnk) { Remove-Item $desktopLnk -Force }
        $smDir = Join-Path ([Environment]::GetFolderPath("Programs")) $AppName
        if (Test-Path $smDir) { Remove-Item -Path $smDir -Recurse -Force }
    }
    Write-Success "$AppName uninstalled successfully."
    exit 0
}

$tempDir = Join-Path $env:TEMP "yago_install_$([Guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory -Path $tempDir -Force | Out-Null

try {
    if ($LocalSource -and (Test-Path $LocalSource)) {
        Write-Info "Using local installation file: $LocalSource"
        $installerPath = $LocalSource
    } else {
        Write-Info "Fetching latest release information from GitHub ($Repo)..."
        $apiUrl = "https://api.github.com/repos/$Repo/releases/latest"
        
        $assetName = if ($Portable) { "yago-portable.exe" } else { "yago-setup.exe" }
        $downloadUrl = "https://github.com/$Repo/releases/latest/download/$assetName"

        try {
            $release = Invoke-RestMethod -Uri $apiUrl -Headers @{ "User-Agent" = "YAGO-Installer" }
            $matchedAsset = $release.assets | Where-Object { $_.name -like "*$assetName*" } | Select-Object -First 1
            if ($matchedAsset) {
                $downloadUrl = $matchedAsset.browser_download_url
            }
        } catch {
            Write-WarningMsg "Could not query GitHub API; attempting direct download from $downloadUrl"
        }

        $installerPath = Join-Path $tempDir $assetName
        Write-Info "Downloading $assetName..."
        Invoke-WebRequest -Uri $downloadUrl -OutFile $installerPath
    }

    if ($Portable) {
        Write-Info "Installing YAGO Portable to $InstallPath..."
        if (!(Test-Path $InstallPath)) { New-Item -ItemType Directory -Path $InstallPath -Force | Out-Null }
        Copy-Item -Path $installerPath -Destination (Join-Path $InstallPath "yago.exe") -Force

        # Create Desktop Shortcut
        $wshShell = New-Object -ComObject WScript.Shell
        $desktopLnk = Join-Path ([Environment]::GetFolderPath("Desktop")) "$AppName.lnk"
        $shortcut = $wshShell.CreateShortcut($desktopLnk)
        $shortcut.TargetPath = Join-Path $InstallPath "yago.exe"
        $shortcut.WorkingDirectory = $InstallPath
        $shortcut.Save()

        # Create Start Menu Shortcut
        $smDir = Join-Path ([Environment]::GetFolderPath("Programs")) $AppName
        if (!(Test-Path $smDir)) { New-Item -ItemType Directory -Path $smDir -Force | Out-Null }
        $smLnk = Join-Path $smDir "$AppName.lnk"
        $smShortcut = $wshShell.CreateShortcut($smLnk)
        $smShortcut.TargetPath = Join-Path $InstallPath "yago.exe"
        $smShortcut.WorkingDirectory = $InstallPath
        $smShortcut.Save()

        Write-Success "$AppName Portable successfully installed to $InstallPath!"
    } else {
        Write-Info "Running YAGO Setup Installer..."
        Start-Process -FilePath $installerPath -ArgumentList "/S" -Wait
        Write-Success "$AppName successfully installed!"
    }
} finally {
    if (Test-Path $tempDir) {
        Remove-Item -Path $tempDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}
