$commonPath =
    if ($PSScriptRoot) {
        Join-Path $PSScriptRoot 'Common.ps1'
    } else {
        Join-Path (Split-Path -Parent $MyInvocation.MyCommand.Path) 'Common.ps1'
    }

. $commonPath

$data = Get-OssifyLaunchData
$outputDir = Get-OssifyLaunchOutputDir

$accountFile = Join-Path $outputDir 'account-setup.txt'
$accounts = @(
    "X founder display name: $($data.Accounts.X.FounderDisplayName)",
    "X founder bio: $($data.Accounts.X.FounderBio)",
    "X founder handles: $($data.Accounts.X.FounderHandles -join ', ')",
    '',
    "LinkedIn personal headline: $($data.Accounts.LinkedIn.PersonalHeadline)",
    'LinkedIn personal About:',
    $data.Accounts.LinkedIn.PersonalAbout.Trim(),
    '',
    "LinkedIn company name: $($data.Accounts.LinkedIn.CompanyName)",
    "LinkedIn company tagline: $($data.Accounts.LinkedIn.CompanyTagline)",
    "LinkedIn company website: $($data.Accounts.LinkedIn.CompanyWebsite)",
    '',
    "Product Hunt maker bio: $($data.Accounts.ProductHunt.Bio)",
    '',
    "Hacker News about: $($data.Accounts.HackerNews.About)",
    '',
    "Reddit suggested usernames: $($data.Accounts.Reddit.SuggestedUsernames -join ', ')",
    "Reddit bio: $($data.Accounts.Reddit.Bio)"
) -join [Environment]::NewLine

Set-Content -Path $accountFile -Value $accounts -NoNewline

foreach ($key in $data.Messages.Keys) {
    $fileName = '{0}.txt' -f ($key -replace '[^a-zA-Z0-9\-]', '-')
    $path = Join-Path $outputDir $fileName
    Set-Content -Path $path -Value ([string]$data.Messages[$key]) -NoNewline
}

Write-OssifyHeader -Title 'Launch drafts generated'
Write-Host "Output directory: $outputDir" -ForegroundColor Green
Get-ChildItem $outputDir | Sort-Object Name | ForEach-Object {
    Write-Host ('- ' + $_.Name)
}
