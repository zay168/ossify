Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-OssifyScriptDirectory {
    if ($PSScriptRoot) {
        return $PSScriptRoot
    }

    if ($MyInvocation.MyCommand.Path) {
        return Split-Path -Parent $MyInvocation.MyCommand.Path
    }

    if ($PSCommandPath) {
        return Split-Path -Parent $PSCommandPath
    }

    throw 'Unable to resolve the launch script directory.'
}

function Get-OssifyLaunchData {
    $dataPath = Join-Path (Get-OssifyScriptDirectory) 'launch-data.psd1'
    return Import-PowerShellDataFile -Path $dataPath
}

function Get-OssifyRepoRoot {
    return (Resolve-Path (Join-Path (Get-OssifyScriptDirectory) '..\..')).Path
}

function Get-OssifyLaunchOutputDir {
    $root = Get-OssifyRepoRoot
    $dir = Join-Path $root 'docs\launch\generated'
    if (-not (Test-Path $dir)) {
        New-Item -ItemType Directory -Path $dir | Out-Null
    }
    return $dir
}

function Copy-OssifyText {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Text
    )

    if (Get-Command Set-Clipboard -ErrorAction SilentlyContinue) {
        Set-Clipboard -Value $Text
        return
    }

    $Text | clip.exe
}

function Open-OssifyUrl {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Url,
        [switch]$DryRun
    )

    if ($DryRun) {
        Write-Host $Url
        return
    }

    Start-Process $Url | Out-Null
}

function ConvertTo-OssifyComposeUrl {
    param(
        [Parameter(Mandatory = $true)]
        [string]$BaseUrl,
        [Parameter(Mandatory = $true)]
        [string]$Text
    )

    return '{0}?text={1}' -f $BaseUrl, [Uri]::EscapeDataString($Text)
}

function Write-OssifyHeader {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Title
    )

    Write-Host ''
    Write-Host $Title -ForegroundColor Cyan
    Write-Host ('=' * $Title.Length) -ForegroundColor DarkCyan
}

function Get-OssifyMessage {
    param(
        [Parameter(Mandatory = $true)]
        [hashtable]$Data,
        [Parameter(Mandatory = $true)]
        [string]$MessageKey
    )

    if (-not $Data.Messages.ContainsKey($MessageKey)) {
        $available = ($Data.Messages.Keys | Sort-Object) -join ', '
        throw "Unknown message key '$MessageKey'. Available: $available"
    }

    return [string]$Data.Messages[$MessageKey]
}
