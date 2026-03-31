param(
    [string]$Version = $env:OSSIFY_VERSION,
    [string]$InstallDir = $(if ($env:OSSIFY_INSTALL_DIR) { $env:OSSIFY_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "Programs\ossify\bin" }),
    [string]$ToolsDir = $(if ($env:OSSIFY_TOOLS_DIR) { $env:OSSIFY_TOOLS_DIR } else { Join-Path (Split-Path -Parent $InstallDir) "tools\bin" }),
    [string]$ActionlintVersion = $env:OSSIFY_ACTIONLINT_VERSION,
    [switch]$PrintOnly
)

$ErrorActionPreference = "Stop"

$repo = "zay168/ossify"
$assetName = "ossify-x86_64-pc-windows-msvc.zip"
$actionlintRepo = "rhysd/actionlint"

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

function Resolve-LatestGitHubReleaseVersion {
    param(
        [string]$Repository
    )

    $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repository/releases/latest" -Headers @{
        "User-Agent" = "ossify-installer"
        "Accept" = "application/vnd.github+json"
    }

    if (-not $release.tag_name) {
        throw "Could not resolve the latest release version for $Repository."
    }

    return $release.tag_name.TrimStart("v")
}

function Resolve-ActionlintUrl {
    param(
        [string]$RequestedVersion
    )

    $resolvedVersion = if ([string]::IsNullOrWhiteSpace($RequestedVersion)) {
        Resolve-LatestGitHubReleaseVersion -Repository $actionlintRepo
    } else {
        if ($RequestedVersion.StartsWith("v")) { $RequestedVersion.TrimStart("v") } else { $RequestedVersion }
    }

    $asset = "actionlint_${resolvedVersion}_windows_amd64.zip"
    $url = "https://github.com/$actionlintRepo/releases/download/v$resolvedVersion/$asset"

    return @{
        Version = $resolvedVersion
        Asset = $asset
        Url = $url
    }
}

$downloadUrl = Resolve-DownloadUrl -Repository $repo -Asset $assetName -RequestedVersion $Version
$actionlint = Resolve-ActionlintUrl -RequestedVersion $ActionlintVersion

if ($PrintOnly) {
    Write-Host "Download URL: $downloadUrl"
    Write-Host "Install directory: $InstallDir"
    Write-Host "Tools directory: $ToolsDir"
    Write-Host "Actionlint URL: $($actionlint.Url)"
    exit 0
}

$tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("ossify-install-" + [guid]::NewGuid().ToString("N"))
$archivePath = Join-Path $tempRoot $assetName
$extractDir = Join-Path $tempRoot "extract"
$actionlintArchive = Join-Path $tempRoot $actionlint.Asset
$actionlintExtract = Join-Path $tempRoot "actionlint"

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

    Write-Host "Downloading managed workflow engine from $($actionlint.Url)"
    Invoke-WebRequest -Uri $actionlint.Url -OutFile $actionlintArchive
    New-Item -ItemType Directory -Path $actionlintExtract -Force | Out-Null
    Expand-Archive -Path $actionlintArchive -DestinationPath $actionlintExtract -Force

    $actionlintBinary = Join-Path $actionlintExtract "actionlint.exe"
    if (-not (Test-Path -LiteralPath $actionlintBinary)) {
        throw "The downloaded actionlint archive did not contain actionlint.exe."
    }

    New-Item -ItemType Directory -Path $ToolsDir -Force | Out-Null
    Copy-Item -LiteralPath $actionlintBinary -Destination (Join-Path $ToolsDir "actionlint.exe") -Force

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
    Write-Host "Managed tools: $(Join-Path $ToolsDir 'actionlint.exe')"
    Write-Host "Next: ossify version"
} finally {
    Remove-Item -LiteralPath $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
}
