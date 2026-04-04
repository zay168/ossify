param(
    [ValidateSet('github', 'x', 'linkedin', 'reddit', 'producthunt', 'hn', 'all')]
    [string]$Group = 'all',
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
$repoRoot = Get-OssifyRepoRoot
$socialPreview = Join-Path $repoRoot $data.Project.SocialPreviewPath

$groups = @{
    github = @(
        $data.Project.RepoUrl,
        $data.Project.LandingUrl,
        $data.Project.ReleasesUrl,
        'https://github.com/zay168',
        'https://github.com/zay168/ossify/discussions'
    )
    x = @(
        (ConvertTo-OssifyComposeUrl -BaseUrl $data.Accounts.X.ComposeUrl -Text ([string]$data.Messages['x-launch'])),
        $data.Accounts.X.ProfileUrl
    )
    linkedin = @(
        $data.Accounts.LinkedIn.FeedUrl,
        $data.Accounts.LinkedIn.CompanySetupUrl,
        $data.Project.LandingUrl
    )
    reddit = @(
        $data.Accounts.Reddit.SubmitUrl,
        $data.Accounts.Reddit.SelfPromotionGuideUrl,
        $data.Project.RepoUrl
    )
    producthunt = @(
        $data.Accounts.ProductHunt.LaunchGuideUrl,
        $data.Accounts.ProductHunt.NewPostUrl,
        $data.Project.LandingUrl
    )
    hn = @(
        $data.Accounts.HackerNews.GuidelinesUrl,
        $data.Accounts.HackerNews.SubmitUrl,
        $data.Project.RepoUrl
    )
}

$targets =
    if ($Group -eq 'all') {
        $groups.Keys | Sort-Object | ForEach-Object { $groups[$_] } | Select-Object -ExpandProperty *
    } else {
        $groups[$Group]
    }

Write-OssifyHeader -Title "Opening launch tabs: $Group"
foreach ($url in $targets) {
    Open-OssifyUrl -Url $url -DryRun:$DryRun
}

if (Test-Path $socialPreview) {
    if ($DryRun) {
        Write-Host $socialPreview
    } else {
        Start-Process $socialPreview | Out-Null
    }
}

Write-Host ''
if ($DryRun) {
    Write-Host 'Dry run only. No browser tabs were opened.' -ForegroundColor Yellow
} else {
    Write-Host 'Tabs opened. Social preview asset opened too.' -ForegroundColor Green
}
