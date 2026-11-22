param(
  [switch]$Uninstall,
  [string]$LocalZip = ""
)

$ErrorActionPreference = "Stop"

$Repo = "ESAK9781/grex"
$InstallDir = Join-Path $env:LOCALAPPDATA "Grex"
$DesktopShortcut = Join-Path ([Environment]::GetFolderPath("Desktop")) "Grex.lnk"
$StartMenuDir = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs"
$StartMenuShortcut = Join-Path $StartMenuDir "Grex.lnk"

function Write-Step([string]$Text) {
  Write-Host "  [grex] " -ForegroundColor DarkYellow -NoNewline
  Write-Host $Text -ForegroundColor White
}

if ($Uninstall) {
  Write-Host "`n  Uninstalling Grex..." -ForegroundColor Cyan
  if (Test-Path $DesktopShortcut) { Remove-Item $DesktopShortcut -Force }
  if (Test-Path $StartMenuShortcut) { Remove-Item $StartMenuShortcut -Force }
  if (Test-Path $InstallDir) { Remove-Item $InstallDir -Recurse -Force }
  $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
  if ($UserPath) {
    $Cleaned = ($UserPath.Split(';') | Where-Object { $_ -and $_ -ne $InstallDir }) -join ';'
    [Environment]::SetEnvironmentVariable("Path", $Cleaned, "User")
  }
  Write-Host "  Grex has been removed.`n" -ForegroundColor Green
  exit 0
}

Write-Host ""
Write-Host "  =============================================" -ForegroundColor DarkGray
Write-Host "    GREX — Local Rust Agent Software Factory" -ForegroundColor Yellow
Write-Host "  =============================================" -ForegroundColor DarkGray
Write-Host ""

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

$ZipFile = ""
if ($LocalZip -and (Test-Path $LocalZip)) {
  Write-Step "Installing from local package: $LocalZip"
  $ZipFile = $LocalZip
} else {
  Write-Step "Fetching latest release from github.com/$Repo..."
  $Release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -Headers @{ "User-Agent" = "grex-installer" }
  $Portable = $Release.assets | Where-Object { $_.name -match "grex-windows.*\.zip$" } | Select-Object -First 1
  if (-not $Portable) {
    $Installer = $Release.assets | Where-Object { $_.name -match "\.(exe|msi)$" -and $_.name -notmatch "grex-mcp" } | Select-Object -First 1
    if ($Installer) {
      $OutFile = Join-Path $env:TEMP $Installer.name
      Write-Step "Downloading $($Installer.name)..."
      Invoke-WebRequest -Uri $Installer.browser_download_url -OutFile $OutFile
      Write-Step "Launching installer..."
      Start-Process -FilePath $OutFile -Wait
      exit 0
    }
    Write-Error "No Windows release asset found on https://github.com/$Repo/releases"
  }

  $ZipFile = Join-Path $env:TEMP $Portable.name
  Write-Step "Downloading $($Portable.name) ($($Release.tag_name))..."
  Invoke-WebRequest -Uri $Portable.browser_download_url -OutFile $ZipFile
}

Write-Step "Extracting binaries to $InstallDir..."
Expand-Archive -Path $ZipFile -DestinationPath $InstallDir -Force

$GrexExe = Join-Path $InstallDir "grex.exe"
$IconPath = Join-Path $InstallDir "icon.ico"

# Add to User PATH so `grex` and `grex-mcp` work from any terminal
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if (-not $UserPath) { $UserPath = "" }
$Parts = $UserPath.Split(';') | Where-Object { $_ }
if ($Parts -notcontains $InstallDir) {
  Write-Step "Adding $InstallDir to User PATH..."
  $NewPath = ($Parts + $InstallDir) -join ';'
  [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
}
if ($env:Path -notlike "*$InstallDir*") {
  $env:Path = "$env:Path;$InstallDir"
}

# Create Desktop & Start Menu shortcuts
Write-Step "Creating Desktop and Start Menu shortcuts..."
$WshShell = New-Object -ComObject WScript.Shell
foreach ($ShortcutPath in @($DesktopShortcut, $StartMenuShortcut)) {
  $Shortcut = $WshShell.CreateShortcut($ShortcutPath)
  $Shortcut.TargetPath = $GrexExe
  $Shortcut.WorkingDirectory = $env:USERPROFILE
  $Shortcut.Description = "Grex — Local Rust Agent Software Factory"
  if (Test-Path $IconPath) {
    $Shortcut.IconLocation = $IconPath
  } else {
    $Shortcut.IconLocation = "$GrexExe,0"
  }
  $Shortcut.Save()
}

Write-Host ""
Write-Host "  Installed successfully!" -ForegroundColor Green
Write-Host "    Terminal command : grex  (and grex-mcp)" -ForegroundColor Cyan
Write-Host "    Desktop shortcut : $DesktopShortcut" -ForegroundColor Cyan
Write-Host "    Install folder   : $InstallDir" -ForegroundColor DarkGray
Write-Host ""
