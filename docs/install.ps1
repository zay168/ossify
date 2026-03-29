param(
    [string]$Version = $env:OSSIFY_VERSION,
    [string]$InstallDir = $(if ($env:OSSIFY_INSTALL_DIR) { $env:OSSIFY_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "Programs\ossify\bin" }),
    [switch]$PrintOnly
)

$ErrorActionPreference = "Stop"

$repo = "zay168/ossify"
$assetName = "ossify-x86_64-pc-windows-msvc.zip"

function Get-OsArchitecture {
    try {
        $runtimeInformation = [System.Type]::GetType("System.Runtime.InteropServices.RuntimeInformation")
        if ($runtimeInformation) {
            $property = $runtimeInformation.GetProperty("OSArchitecture")
            if ($property) {
                $value = $property.GetValue($null, @())
                if ($value) {
                    return $value.ToString()
                }
            }
        }
    } catch {
    }

    if ([Environment]::Is64BitOperatingSystem) {
        return "X64"
    }

    $architecture = $env:PROCESSOR_ARCHITEW6432
    if ([string]::IsNullOrWhiteSpace($architecture)) {
        $architecture = $env:PROCESSOR_ARCHITECTURE
    }

    if ([string]::IsNullOrWhiteSpace($architecture)) {
        return "Unknown"
    }

    if ($architecture -match "64") {
        return "X64"
    }

    return $architecture
}

$architecture = Get-OsArchitecture
if ($architecture -ne "X64") {
    throw "This installer currently ships Windows builds for x64 only. Detected architecture: $architecture."
}

function Resolve-DownloadUrl {
    param(
        [string]$Repository,
        [string]$Asset,
        [string]$RequestedVersion
    )

    if ([string]::IsNullOrWhiteSpace($RequestedVersion)) {
        return "https://github.com/$Repository/releases/latest/download/$Asset"
    }

    $tag = if ($RequestedVersion.StartsWith("v")) { $RequestedVersion } else { "v$RequestedVersion" }
    return "https://github.com/$Repository/releases/download/$tag/$Asset"
}

$downloadUrl = Resolve-DownloadUrl -Repository $repo -Asset $assetName -RequestedVersion $Version

if ($PrintOnly) {
    Write-Host "Download URL: $downloadUrl"
    Write-Host "Install directory: $InstallDir"
    exit 0
}

$tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("ossify-install-" + [guid]::NewGuid().ToString("N"))
$archivePath = Join-Path $tempRoot $assetName
$extractDir = Join-Path $tempRoot "extract"

New-Item -ItemType Directory -Path $tempRoot -Force | Out-Null

try {
    Write-Host "Downloading ossify from $downloadUrl"
    Invoke-WebRequest -Uri $downloadUrl -OutFile $archivePath

    New-Item -ItemType Directory -Path $extractDir -Force | Out-Null
    Expand-Archive -Path $archivePath -DestinationPath $extractDir -Force

    $binaryPath = Join-Path $extractDir "ossify.exe"
    if (-not (Test-Path -LiteralPath $binaryPath)) {
        throw "The downloaded archive did not contain ossify.exe."
    }

    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    Copy-Item -LiteralPath $binaryPath -Destination (Join-Path $InstallDir "ossify.exe") -Force

    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $pathEntries = @()
    if (-not [string]::IsNullOrWhiteSpace($userPath)) {
        $pathEntries = $userPath.Split(";", [System.StringSplitOptions]::RemoveEmptyEntries)
    }

    $installDirFull = [System.IO.Path]::GetFullPath($InstallDir)
    $alreadyOnPath = $pathEntries | Where-Object { [System.IO.Path]::GetFullPath($_) -eq $installDirFull }

    if (-not $alreadyOnPath) {
        $newUserPath = if ([string]::IsNullOrWhiteSpace($userPath)) {
            $installDirFull
        } else {
            "$userPath;$installDirFull"
        }

        [Environment]::SetEnvironmentVariable("Path", $newUserPath, "User")

        if (-not ($env:Path.Split(";") | Where-Object { $_ -eq $installDirFull })) {
            $env:Path = "$env:Path;$installDirFull"
        }

        Write-Host "Added $installDirFull to your user PATH."
    }

    Write-Host ""
    Write-Host "ossify installed successfully."
    Write-Host "Binary: $(Join-Path $InstallDir 'ossify.exe')"
    Write-Host "Next: ossify version"
} finally {
    Remove-Item -LiteralPath $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
}
