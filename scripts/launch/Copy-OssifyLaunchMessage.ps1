param(
    [Parameter(Mandatory = $true)]
    [string]$MessageKey,
    [switch]$NoClipboard
)

$commonPath =
    if ($PSScriptRoot) {
        Join-Path $PSScriptRoot 'Common.ps1'
    } else {
        Join-Path (Split-Path -Parent $MyInvocation.MyCommand.Path) 'Common.ps1'
    }

. $commonPath

$data = Get-OssifyLaunchData
$message = Get-OssifyMessage -Data $data -MessageKey $MessageKey

if (-not $NoClipboard) {
    Copy-OssifyText -Text $message
}

Write-OssifyHeader -Title "Message: $MessageKey"
Write-Host $message

if (-not $NoClipboard) {
    Write-Host ''
    Write-Host 'Copied to clipboard.' -ForegroundColor Green
}
