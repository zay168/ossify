@{
    Project = @{
        Name = 'ossify'
        Founder = 'Zayd'
        RepoUrl = 'https://github.com/zay168/ossify'
        LandingUrl = 'https://ossify-react.netlify.app/ossify/'
        ReleasesUrl = 'https://github.com/zay168/ossify/releases'
        SocialPreviewPath = 'docs/assets/ossify-social-preview.png'
    }

    Accounts = @{
        X = @{
            FounderDisplayName = 'Zayd | ossify'
            FounderHandles = @('@zay168', '@zaydbuilds', '@zaydships')
            FounderBio = 'Building ossify, a Rust CLI that audits repo trust signals, workflow hygiene, and the missing files that make open-source projects feel ready to adopt.'
            BrandDisplayName = 'ossify'
            BrandHandles = @('@ossifydev', '@ossify_cli', '@tryossify')
            BrandBio = 'Repo trust doctor for open-source projects. Audit signals, workflow hygiene, docs, and release readiness.'
            Link = 'https://ossify-react.netlify.app/ossify/'
            SignupUrl = 'https://x.com/i/flow/signup'
            ProfileUrl = 'https://x.com/settings/profile'
            ComposeUrl = 'https://x.com/compose/post'
        }

        LinkedIn = @{
            PersonalHeadline = 'Builder of ossify | Rust CLI for repository trust, workflow hygiene, and open-source readiness'
            PersonalAbout = @'
I build developer tools that make repositories easier to trust, adopt, and contribute to.

Currently building ossify: a Rust CLI that audits repo trust signals, checks workflow hygiene, and scaffolds the missing files that make an open-source project feel ready.
'@
            CompanyName = 'ossify'
            CompanyTagline = 'Repository trust signals, workflow hygiene, and open-source readiness.'
            CompanyWebsite = 'https://ossify-react.netlify.app/ossify/'
            FeedUrl = 'https://www.linkedin.com/feed/'
            ProfileEditUrl = 'https://www.linkedin.com/in/'
            CompanySetupUrl = 'https://www.linkedin.com/company/setup/new/'
        }

        ProductHunt = @{
            DisplayName = 'Zayd'
            Bio = 'Building developer tools for repo trust, workflow hygiene, and open-source readiness.'
            LoginUrl = 'https://www.producthunt.com/login'
            LaunchGuideUrl = 'https://www.producthunt.com/launch'
            NewPostUrl = 'https://www.producthunt.com/posts/new'
        }

        HackerNews = @{
            About = 'Building open-source developer tools. Currently working on ossify, a Rust CLI for repository trust signals and workflow hygiene.'
            LoginUrl = 'https://news.ycombinator.com/login?goto=news'
            SubmitUrl = 'https://news.ycombinator.com/submit'
            GuidelinesUrl = 'https://news.ycombinator.com/newsguidelines.html'
        }

        Reddit = @{
            SuggestedUsernames = @('zayd_builds', 'zayd_dev', 'repo_doctor')
            Bio = 'Building ossify, a Rust CLI for repo trust signals, workflow hygiene, and open-source readiness. Here to share the work and learn from maintainers.'
            SignupUrl = 'https://www.reddit.com/register/'
            SubmitUrl = 'https://www.reddit.com/submit'
            SelfPromotionGuideUrl = 'https://www.reddit.com/r/reddit.com/wiki/selfpromotion/'
        }
    }

    Messages = @{
        'x-launch' = @'
Most repos that "work" still do not feel ready to adopt.

I built ossify to audit the trust layer around a repository:
- docs
- metadata
- workflow hygiene
- release surface
- the missing files that make a project feel maintained

It is a Rust CLI, and it can both audit and scaffold fixes.

Repo: https://github.com/zay168/ossify
Landing: https://ossify-react.netlify.app/ossify/
'@

        'x-thread-1' = @'
I built ossify: a Rust CLI that helps turn rough repos into projects people trust on sight.

GitHub: https://github.com/zay168/ossify
Landing: https://ossify-react.netlify.app/ossify/
'@

        'x-thread-2' = @'
The idea is simple:

a repo can be technically correct and still feel risky to adopt.

Usually the gaps are subtle:
- weak docs
- workflow issues
- missing maintainer files
- unclear release surface
'@

        'x-thread-3' = @'
ossify audits those signals and gives you a cleaner view of what is missing.

It also scaffolds the files and structure that make a project feel more real to contributors and users.
'@

        'x-thread-4' = @'
I have been tightening things like:
- workflow doctoring
- launch-ready project formalities
- install flow
- landing page clarity
- repo trust signals
'@

        'x-thread-5' = @'
If you maintain open-source repos, I would love feedback on what makes a project feel adoptable versus not quite there yet.
'@

        'linkedin-personal' = @'
Most repositories are judged long before anyone reads them deeply.

A project can compile, ship, and even have users, while still feeling difficult to trust or adopt from the outside.

That is the problem I wanted to work on with ossify.

ossify is a Rust CLI that audits the trust layer around a repository:
- docs
- metadata
- workflow hygiene
- maintainer-facing formalities
- release readiness

The goal is not just to point out gaps, but to help scaffold the missing pieces that make an open-source project feel ready.

GitHub:
https://github.com/zay168/ossify

Landing page:
https://ossify-react.netlify.app/ossify/

If you maintain repos or evaluate open-source projects often, I would love to know what signals make you trust a project quickly.
'@

        'linkedin-company' = @'
ossify is now live.

It is a Rust CLI built to audit repository trust signals, workflow hygiene, and the missing files that make an open-source project feel ready to adopt.

Explore the project:
https://ossify-react.netlify.app/ossify/

GitHub:
https://github.com/zay168/ossify
'@

        'producthunt-tagline' = 'Audit repo trust signals and scaffold open-source readiness'

        'producthunt-description' = 'A Rust CLI that audits workflow hygiene, maintainer signals, docs, and release readiness so repositories feel ready to adopt.'

        'producthunt-comment' = @'
Hey Product Hunt, I am Zayd, and I built ossify because I kept running into repositories that were technically good, but still felt hard to trust at first glance.

The problem usually was not a single bug. It was the missing trust layer around the codebase:
- workflow hygiene
- maintainer-facing docs
- metadata
- release surface
- project formalities that make a repo feel cared for

ossify is a Rust CLI that audits those signals and helps scaffold what is missing.

What I would especially love feedback on:
- which repo signals matter most to you before adopting a project
- where the current CLI feels too strict or not strict enough
- what would make this more useful in real maintainer workflows
'@

        'show-hn-title' = 'Show HN: ossify - a Rust CLI for repo trust signals and workflow hygiene'

        'show-hn-body' = @'
I built ossify to help with a problem I keep noticing in open-source projects:

a repo can work perfectly fine and still feel difficult to trust or adopt.

The gaps are usually not deep architectural bugs. They are the missing maintainer signals around the codebase:
- docs
- metadata
- workflow hygiene
- release surface
- files that make the project feel cared for

ossify is a Rust CLI that audits those signals and can scaffold missing pieces.

I would especially love feedback on:
- which signals feel most important before adopting a repo
- whether the workflow checks are calibrated correctly
- where it feels opinionated in the wrong way

Repo: https://github.com/zay168/ossify
Landing: https://ossify-react.netlify.app/ossify/
'@

        'reddit-post' = @'
I have been working on a Rust CLI called ossify.

The idea is to help make repositories feel easier to trust and adopt by auditing the layer around the codebase: docs, maintainer files, workflow hygiene, release readiness, and similar signals.

I am not mainly looking for upvotes here - I would genuinely like feedback from people who maintain repos or evaluate tools often:

- what makes a repo feel trustworthy quickly?
- which signals matter most to you?
- what do most tools miss?

Repo: https://github.com/zay168/ossify
Landing: https://ossify-react.netlify.app/ossify/
'@
    }
}
