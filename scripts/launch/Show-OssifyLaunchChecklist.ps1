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

Write-OssifyHeader -Title 'ossify launch checklist'

$items = @(
    "[ ] GitHub repo looks correct: $($data.Project.RepoUrl)",
    "[ ] Landing is live: $($data.Project.LandingUrl)",
    "[ ] Releases page is clean: $($data.Project.ReleasesUrl)",
    "[ ] Social preview asset exists: $socialPreview",
    "[ ] X founder account uses display name '$($data.Accounts.X.FounderDisplayName)'",
    "[ ] LinkedIn personal headline updated",
    "[ ] LinkedIn company page 'ossify' created or updated",
    "[ ] Product Hunt maker account ready",
    "[ ] Hacker News account ready and guidelines reviewed",
    "[ ] Reddit account ready and subreddit rules checked",
    "[ ] X launch post copied and reviewed",
    "[ ] LinkedIn personal post copied and reviewed",
    "[ ] Show HN title and body copied and reviewed",
    "[ ] Reddit post adapted to the target subreddit",
    "[ ] Product Hunt tagline, description, and first comment prepared"
) 

foreach ($item in $items) {
    Write-Host $item
}

Write-Host ''
Write-Host 'Helpful commands:' -ForegroundColor Cyan
Write-Host '  powershell -File scripts/launch/New-OssifyLaunchDrafts.ps1'
Write-Host '  powershell -File scripts/launch/Copy-OssifyLaunchMessage.ps1 -MessageKey x-launch'
Write-Host '  powershell -File scripts/launch/Open-OssifyLaunchTabs.ps1 -Group x'
Write-Host '  powershell -File scripts/launch/Open-OssifyAccountSetup.ps1 -Platform linkedin'
