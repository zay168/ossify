param(
    [ValidateSet('x', 'linkedin', 'producthunt', 'reddit', 'hn', 'all')]
    [string]$Platform = 'all',
    [switch]$DryRun
)

$commonPath =
    if ($PSScriptRoot) {
        Join-Path $PSScriptRoot 'Common.ps1'
    } else {
        Join-Path (Split-Path -Parent $MyInvocation.MyCommand.Path) 'Common.ps1'
    }

. $commonPath

$data = Get-OssifyLaunchData

$accountUrls = @{
    x = @(
        $data.Accounts.X.SignupUrl,
        $data.Accounts.X.ProfileUrl
    )
    linkedin = @(
        $data.Accounts.LinkedIn.FeedUrl,
        $data.Accounts.LinkedIn.CompanySetupUrl
    )
    producthunt = @(
        $data.Accounts.ProductHunt.LoginUrl,
        $data.Accounts.ProductHunt.LaunchGuideUrl
    )
    reddit = @(
        $data.Accounts.Reddit.SignupUrl,
        $data.Accounts.Reddit.SubmitUrl
    )
    hn = @(
        $data.Accounts.HackerNews.LoginUrl,
        $data.Accounts.HackerNews.GuidelinesUrl
    )
}

$targets =
    if ($Platform -eq 'all') {
        $accountUrls.Keys | Sort-Object | ForEach-Object { $accountUrls[$_] } | Select-Object -ExpandProperty *
    } else {
        $accountUrls[$Platform]
    }

Write-OssifyHeader -Title "Account setup: $Platform"
foreach ($url in $targets) {
    Open-OssifyUrl -Url $url -DryRun:$DryRun
}

$accountSummary = @(
    "X display name: $($data.Accounts.X.FounderDisplayName)",
    "X bio: $($data.Accounts.X.FounderBio)",
    '',
    "LinkedIn headline: $($data.Accounts.LinkedIn.PersonalHeadline)",
    'LinkedIn About:',
    $data.Accounts.LinkedIn.PersonalAbout.Trim(),
    '',
    "LinkedIn company: $($data.Accounts.LinkedIn.CompanyName)",
    "LinkedIn tagline: $($data.Accounts.LinkedIn.CompanyTagline)",
    '',
    "Product Hunt bio: $($data.Accounts.ProductHunt.Bio)",
    '',
    "Hacker News about: $($data.Accounts.HackerNews.About)",
    '',
    "Reddit usernames: $($data.Accounts.Reddit.SuggestedUsernames -join ', ')",
    "Reddit bio: $($data.Accounts.Reddit.Bio)"
) -join [Environment]::NewLine

Copy-OssifyText -Text $accountSummary

Write-Host ''
Write-Host 'Account setup summary copied to clipboard.' -ForegroundColor Green
