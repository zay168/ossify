"use client";

import React from "react";
import {
  ArrowRight,
  ChevronRight,
  Menu,
  X,
} from "lucide-react";

import { AnimatedGroup } from "@/components/ui/animated-group";
import { AnimatedTextCycleDemo } from "@/components/ui/animated-text-cycle-demo";
import { Button } from "@/components/ui/button";
import { FlickeringFooter } from "@/components/ui/flickering-footer";
import { cn } from "@/lib/utils";

const transitionVariants = {
  item: {
    hidden: {
      opacity: 0,
      filter: "blur(12px)",
      y: 12,
    },
    visible: {
      opacity: 1,
      filter: "blur(0px)",
      y: 0,
      transition: {
        type: "spring" as const,
        bounce: 0.3,
        duration: 1.5,
      },
    },
  },
};

const menuItems = [
  { name: "Install", href: "#install" },
  { name: "Signals", href: "#signals" },
  { name: "GitHub", href: "https://github.com/zay168/ossify" },
];

const trustSignals = [
  {
    eyebrow: "Docs",
    title: "README and metadata stop feeling incidental",
    body: "Identity, ownership, and security cues read like deliberate maintainer choices.",
  },
  {
    eyebrow: "Workflow",
    title: "CI and permissions look maintained",
    body: "GitHub Actions, token scope, and release hygiene are visible before deeper review.",
  },
  {
    eyebrow: "Plan",
    title: "Safe fixes stay previewable",
    body: "Scaffold missing files conservatively and keep manual edits explicit.",
  },
  {
    eyebrow: "Coverage",
    title: "One CLI spans the trust layer",
    body: "Audit the repo once, then drill into workflow, deps, docs, and release with the same vocabulary.",
  },
];

const commandRail = [
  { title: "Audit", command: "ossify audit ." },
  { title: "Workflow", command: "ossify workflow ." },
  { title: "Deps", command: "ossify deps ." },
  { title: "Release", command: "ossify release ." },
];


const terminalFindings = [
  {
    tone: "text-emerald-200",
    badge: "docs",
    text: "README surface is present and readable",
  },
  {
    tone: "text-sky-200",
    badge: "workflow",
    text: "actionlint and permissions checks passed",
  },
  {
    tone: "text-amber-200",
    badge: "deps",
    text: "paste 1.0.15 flagged as unmaintained (RUSTSEC-2024-0436)",
  },
  {
    tone: "text-slate-300",
    badge: "release",
    text: "GitHub release notes still need editorial framing",
  },
];

const terminalScores = [
  { label: "docs", value: "92" },
  { label: "workflow", value: "100" },
  { label: "deps", value: "72" },
  { label: "release", value: "94" },
];

const scaffoldPreview = [
  { status: "created", label: "CHANGELOG.md" },
  { status: "created", label: ".github/workflows/ci.yml" },
  { status: "manual", label: "Release notes framing" },
];

const installHost = "https://ossify-react.netlify.app";

export function HeroSection() {
  return (
    <>
      <HeroHeader />
      <main id="top" className="overflow-hidden">
        <div
          aria-hidden
          className="pointer-events-none absolute inset-0 isolate hidden opacity-60 lg:block"
        >
          <div className="absolute left-0 top-0 h-[80rem] w-[35rem] -translate-y-[350px] -rotate-45 rounded-full bg-[radial-gradient(68.54%_68.72%_at_55.02%_31.46%,hsla(196,100%,82%,0.12)_0,hsla(196,100%,70%,0.03)_50%,transparent_80%)]" />
          <div className="absolute left-0 top-0 h-[80rem] w-56 -translate-y-[280px] -rotate-45 rounded-full bg-[radial-gradient(50%_50%_at_50%_50%,hsla(44,91%,69%,0.14)_0,hsla(44,91%,69%,0.02)_80%,transparent_100%)]" />
        </div>

        <section>
          <div className="relative pt-24 md:pt-36">
            <div aria-hidden className="absolute inset-0 -z-20 overflow-hidden">
              <div className="absolute inset-x-[8%] top-20 h-[32rem] rounded-[3rem] bg-[radial-gradient(circle_at_top,rgba(125,211,252,0.14),transparent_28%),linear-gradient(180deg,rgba(14,21,37,0.55),rgba(8,13,24,0.05))]" />
              <div className="absolute left-[18%] top-[19rem] size-2 rounded-full bg-primary shadow-[0_0_18px_rgba(125,211,252,0.6)]" />
              <div className="absolute left-[41%] top-[23rem] size-2 rounded-full bg-[#f4bf72] shadow-[0_0_18px_rgba(244,191,114,0.5)]" />
              <div className="absolute right-[23%] top-[16.5rem] size-2 rounded-full bg-primary shadow-[0_0_18px_rgba(125,211,252,0.6)]" />
              <div className="absolute left-[52%] top-[10rem] h-24 w-24 rounded-full bg-[radial-gradient(circle,rgba(244,191,114,0.18),transparent_70%)] blur-2xl" />
            </div>

            <div
              aria-hidden
              className="absolute inset-0 -z-10 h-full w-full bg-[radial-gradient(125%_125%_at_50%_100%,transparent_0%,hsl(var(--background))_75%)]"
            />

            <div className="mx-auto max-w-7xl px-6">
              <div className="text-center">
                <div>
                  <a
                    href="#install"
                    className="group mx-auto flex w-fit items-center gap-4 rounded-full bg-muted/70 p-1 pl-4 shadow-[0_10px_30px_rgba(0,0,0,0.12)] transition-all duration-300 hover:bg-background/80"
                  >
                    <span className="text-sm text-foreground">
                      Native install for PowerShell and shell
                    </span>
                    <span className="block h-4 w-px bg-white/12" />
                    <div className="size-6 overflow-hidden rounded-full bg-background transition-colors duration-500 group-hover:bg-muted">
                      <div className="flex w-12 -translate-x-1/2 transition-transform duration-500 ease-in-out group-hover:translate-x-0">
                        <span className="flex size-6">
                          <ArrowRight className="m-auto size-3" />
                        </span>
                        <span className="flex size-6">
                          <ArrowRight className="m-auto size-3" />
                        </span>
                      </div>
                    </div>
                  </a>

                  <h1 className="mx-auto mt-8 max-w-5xl text-balance font-serif text-6xl tracking-[-0.06em] md:text-7xl lg:mt-16 xl:text-[5.25rem]">
                    Turn rough repos into projects people trust on sight.
                  </h1>

                  <p className="mx-auto mt-7 max-w-3xl text-[11px] uppercase tracking-[0.32em] text-muted-foreground">
                    docs / workflow / deps / release / safe plan
                  </p>

                  <p className="mx-auto mt-8 max-w-2xl text-balance text-lg leading-8 text-muted-foreground">
                    Audit the trust layer around your repo, surface the gaps that actually
                    matter, and preview safe fixes before you touch the tree.
                  </p>

                  <div className="mt-9 flex justify-center overflow-hidden px-6">
                    <AnimatedTextCycleDemo />
                  </div>

                </div>

                <div className="mt-12 flex flex-col items-center justify-center gap-2 md:flex-row">
                  <div className="rounded-[14px] bg-foreground/10 p-0.5 shadow-[0_10px_25px_rgba(0,0,0,0.12)]">
                    <Button asChild size="lg" className="rounded-xl px-5 text-base">
                      <a href="#install">
                        <span className="text-nowrap">Install ossify</span>
                      </a>
                    </Button>
                  </div>
                  <Button
                    asChild
                    size="lg"
                    variant="ghost"
                    className="h-[2.625rem] rounded-xl px-5"
                  >
                    <a href="https://github.com/zay168/ossify">
                      <span className="text-nowrap">View the repo</span>
                    </a>
                  </Button>
                </div>

                <div className="relative mt-10 overflow-hidden px-2 sm:mt-12 md:mt-16">
                  <div
                    aria-hidden
                    className="absolute inset-0 z-10 bg-gradient-to-b from-transparent from-30% to-background"
                  />
                  <div className="relative mx-auto max-w-6xl overflow-hidden rounded-[1.75rem] bg-background/90 p-4 shadow-[0_30px_80px_rgba(0,0,0,0.35)]">
                    <div className="relative overflow-hidden rounded-[1.35rem] bg-[linear-gradient(180deg,rgba(10,18,32,0.98),rgba(8,14,24,0.98))] p-4 md:p-5 shadow-[inset_0_1px_0_rgba(255,255,255,0.04)]">
                      <div className="absolute inset-0 bg-[radial-gradient(circle_at_top_left,rgba(125,211,252,0.12),transparent_32%),radial-gradient(circle_at_bottom_right,rgba(251,191,36,0.1),transparent_22%)]" />
                      <div className="relative">
                        <div className="flex flex-wrap items-center justify-between gap-3 border-b border-white/8 pb-4">
                          <div className="flex items-center gap-2">
                            <span className="size-2.5 rounded-full bg-[#f87171]" />
                            <span className="size-2.5 rounded-full bg-[#fbbf24]" />
                            <span className="size-2.5 rounded-full bg-[#34d399]" />
                            <span className="ml-3 font-mono text-xs uppercase tracking-[0.22em] text-muted-foreground">
                              maintainer pass
                            </span>
                          </div>
                          <div className="flex flex-wrap items-center justify-end gap-2">
                            {commandRail.map((item) => (
                              <span
                                key={item.title}
                                className="rounded-full bg-white/[0.04] px-3 py-1 font-mono text-[11px] uppercase tracking-[0.18em] text-muted-foreground"
                              >
                                {item.title}
                              </span>
                            ))}
                          </div>
                        </div>

                        <div className="mt-5 grid gap-5 lg:grid-cols-[1.25fr_0.75fr]">
                          <div className="rounded-[1.15rem] bg-black/25 px-4 py-4 text-left shadow-[inset_0_1px_0_rgba(255,255,255,0.03),0_10px_30px_rgba(0,0,0,0.12)]">
                            <div className="font-mono text-[12px] leading-6 text-slate-300 md:text-[13px]">
                              <div className="text-primary">
                                PS C:\repo&gt; <span className="text-foreground">ossify audit .</span>
                              </div>
                              <div className="mt-2 text-slate-200">
                                score 84/100  tier promising  docs 92  workflow 100  deps 72  release 94
                              </div>
                              <div className="mt-3 text-[11px] uppercase tracking-[0.18em] text-muted-foreground">
                                dominant signals
                              </div>
                              <div className="mt-2 space-y-2">
                                {terminalFindings.map((finding) => (
                                  <div key={finding.text} className="flex gap-3">
                                    <span className={cn("w-14 shrink-0 uppercase", finding.tone)}>
                                      {finding.badge}
                                    </span>
                                    <span className="text-slate-300">{finding.text}</span>
                                  </div>
                                ))}
                              </div>
                              <div className="mt-4 text-primary">
                                PS C:\repo&gt; <span className="text-foreground">ossify plan .</span>
                              </div>
                              <div className="mt-2 text-slate-300">
                                current 63 {"->"} estimated 91  |  3 scaffold actions  |  1 manual note
                              </div>
                            </div>
                          </div>

                          <div className="grid gap-4">
                            <div className="rounded-[1.15rem] bg-white/[0.03] p-4 text-left shadow-[0_10px_30px_rgba(0,0,0,0.1)]">
                              <p className="text-[11px] uppercase tracking-[0.22em] text-primary">
                                domain scores
                              </p>
                              <div className="mt-4 grid gap-2">
                                {terminalScores.map((score) => (
                                  <div
                                    key={score.label}
                                    className="flex items-center justify-between rounded-xl bg-black/20 px-3 py-2"
                                  >
                                    <span className="font-mono text-[11px] uppercase tracking-[0.18em] text-muted-foreground">
                                      {score.label}
                                    </span>
                                    <span className="font-mono text-sm text-foreground">
                                      {score.value}/100
                                    </span>
                                  </div>
                                ))}
                              </div>
                            </div>

                            <div className="rounded-[1.15rem] bg-white/[0.03] p-4 text-left shadow-[0_10px_30px_rgba(0,0,0,0.1)]">
                              <p className="text-[11px] uppercase tracking-[0.22em] text-primary">
                                safe scaffolds
                              </p>
                              <div className="mt-4 space-y-2">
                                {scaffoldPreview.map((item) => (
                                  <div
                                    key={item.label}
                                    className="flex items-center justify-between gap-4 rounded-xl bg-black/20 px-3 py-2"
                                  >
                                    <span
                                      className={cn(
                                        "rounded-full px-2 py-1 font-mono text-[10px] uppercase tracking-[0.18em]",
                                        item.status === "created" && "bg-emerald-400/14 text-emerald-200",
                                        item.status === "manual" && "bg-sky-300/12 text-sky-200",
                                      )}
                                    >
                                      {item.status}
                                    </span>
                                    <span className="text-right text-sm text-muted-foreground">
                                      {item.label}
                                    </span>
                                  </div>
                                ))}
                              </div>
                            </div>
                          </div>
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </section>

        <section
          id="signals"
          className="bg-background pb-16 pt-16 md:pb-32 [content-visibility:auto] [contain-intrinsic-size:720px]"
        >
          <div className="mx-auto max-w-6xl px-6">
            <div className="grid gap-10 lg:grid-cols-[0.88fr_1.12fr] lg:items-start">
              <AnimatedGroup variants={transitionVariants}>
                <div className="rounded-[1.75rem] border border-white/10 bg-[linear-gradient(180deg,rgba(14,21,37,0.82),rgba(8,13,24,0.92))] p-8">
                  <p className="text-xs uppercase tracking-[0.28em] text-primary">
                    Trust surfaces
                  </p>
                  <h2 className="mt-4 max-w-md font-serif text-4xl tracking-[-0.05em]">
                    What ossify reads before a repo feels safe to adopt.
                  </h2>
                  <p className="mt-5 max-w-lg text-lg leading-8 text-muted-foreground">
                    Healthy repositories signal intent in more than one place. Docs,
                    workflow discipline, dependency policy, and release readiness need to
                    reinforce each other instead of sending mixed messages.
                  </p>
                  <a
                    href="#flow"
                    className="mt-6 inline-flex items-center gap-2 text-sm text-foreground transition-opacity duration-150 hover:opacity-75"
                  >
                    <span>See the install path</span>
                    <ChevronRight className="size-4" />
                  </a>
                </div>
              </AnimatedGroup>

              <AnimatedGroup
                variants={{
                  container: {
                    visible: {
                      transition: {
                        staggerChildren: 0.06,
                        delayChildren: 0.15,
                      },
                    },
                  },
                  ...transitionVariants,
                }}
                className="grid gap-4 sm:grid-cols-2"
              >
                {trustSignals.map((signal) => (
                  <div key={signal.title} className="border-t border-white/10 pt-4">
                    <p className="text-[11px] uppercase tracking-[0.24em] text-primary">
                      {signal.eyebrow}
                    </p>
                    <p className="mt-2 text-lg font-medium tracking-[-0.02em] text-foreground">
                      {signal.title}
                    </p>
                    <p className="mt-2 text-sm leading-7 text-muted-foreground">
                      {signal.body}
                    </p>
                  </div>
                ))}
              </AnimatedGroup>
            </div>
          </div>
        </section>

        <section
          id="install"
          className="mx-auto max-w-6xl px-6 pb-24 [content-visibility:auto] [contain-intrinsic-size:760px]"
        >
          <div className="mx-auto max-w-4xl border-t border-white/8 pt-14 text-center">
            <div className="mx-auto flex flex-col items-center">
              <p className="text-xs uppercase tracking-[0.28em] text-primary">Install</p>
              <h2 className="mx-auto mt-4 max-w-[42rem] text-balance font-serif text-4xl tracking-[-0.05em]">
                One command. One public path.
              </h2>
              <p className="mx-auto mt-4 max-w-[42rem] text-balance leading-8 text-muted-foreground">
                The landing, installer URLs, and shipped binary now point to the same
                delivery surface.
              </p>

              <div id="flow" className="mx-auto mt-10 w-full max-w-[42rem] space-y-6 text-left">
                <div>
                  <p className="text-xs uppercase tracking-[0.24em] text-muted-foreground">
                    Windows PowerShell
                  </p>
                  <code className="mt-3 block overflow-x-auto rounded-2xl bg-black/35 px-4 py-4 text-sm text-primary shadow-[inset_0_1px_0_rgba(255,255,255,0.03)]">
                    {`irm ${installHost}/install.ps1 | iex`}
                  </code>
                </div>

                <div>
                  <p className="text-xs uppercase tracking-[0.24em] text-muted-foreground">
                    macOS / Linux
                  </p>
                  <code className="mt-3 block overflow-x-auto rounded-2xl bg-black/35 px-4 py-4 text-sm text-primary shadow-[inset_0_1px_0_rgba(255,255,255,0.03)]">
                    {`curl -fsSL ${installHost}/install.sh | sh`}
                  </code>
                </div>
              </div>

              <ul className="mx-auto mt-8 grid w-full max-w-[42rem] gap-3 text-sm text-muted-foreground sm:grid-cols-3">
                <li className="text-center">native CLI on the host</li>
                <li className="text-center">managed workflow checks immediately</li>
                <li className="text-center">deps and release engines on first use</li>
              </ul>
            </div>
          </div>
        </section>
        <FlickeringFooter />
      </main>
    </>
  );
}

const HeroHeader = () => {
  const [menuState, setMenuState] = React.useState(false);
  const [isScrolled, setIsScrolled] = React.useState(false);

  React.useEffect(() => {
    const handleScroll = () => {
      setIsScrolled(window.scrollY > 40);
    };

    window.addEventListener("scroll", handleScroll, { passive: true });
    return () => window.removeEventListener("scroll", handleScroll);
  }, []);

  return (
    <header>
      <nav data-state={menuState ? "active" : "inactive"} className="group fixed z-20 w-full px-2">
        <div
          className={cn(
            "mx-auto mt-2 max-w-6xl px-4 transition-all duration-300 lg:px-8",
            isScrolled && "max-w-4xl rounded-[1.3rem] bg-background/65 shadow-[0_14px_40px_rgba(0,0,0,0.12)] backdrop-blur-xl",
          )}
        >
          <div className="relative flex flex-wrap items-center justify-between gap-6 py-3 lg:gap-0 lg:py-4">
            <div className="flex w-full justify-between lg:w-auto">
              <a href="#top" aria-label="home" className="flex items-center space-x-3">
                <Logo />
              </a>

              <button
                onClick={() => setMenuState(!menuState)}
                aria-label={menuState ? "Close Menu" : "Open Menu"}
                className="relative z-20 -m-2.5 -mr-4 block cursor-pointer p-2.5 lg:hidden"
              >
                <Menu className="m-auto size-6 transition-all duration-200 group-data-[state=active]:scale-0 group-data-[state=active]:rotate-180 group-data-[state=active]:opacity-0" />
                <X className="absolute inset-0 m-auto size-6 -rotate-180 scale-0 opacity-0 transition-all duration-200 group-data-[state=active]:rotate-0 group-data-[state=active]:scale-100 group-data-[state=active]:opacity-100" />
              </button>
            </div>

            <div className="absolute inset-0 m-auto hidden size-fit lg:block">
              <ul className="flex gap-8 text-sm">
                {menuItems.map((item) => (
                  <li key={item.name}>
                    <a
                      href={item.href}
                      className="block text-muted-foreground transition-colors duration-150 hover:text-foreground"
                    >
                      <span>{item.name}</span>
                    </a>
                  </li>
                ))}
              </ul>
            </div>

            <div className="mb-6 hidden w-full flex-wrap items-center justify-end space-y-8 rounded-[1.75rem] bg-background p-6 shadow-2xl shadow-zinc-950/20 md:flex-nowrap lg:m-0 lg:flex lg:w-fit lg:gap-3 lg:space-y-0 lg:bg-transparent lg:p-0 lg:shadow-none group-data-[state=active]:block lg:group-data-[state=active]:flex">
              <div className="lg:hidden">
                <ul className="space-y-6 text-base">
                  {menuItems.map((item) => (
                    <li key={item.name}>
                      <a
                        href={item.href}
                        onClick={() => setMenuState(false)}
                        className="block text-muted-foreground transition-colors duration-150 hover:text-foreground"
                      >
                        <span>{item.name}</span>
                      </a>
                    </li>
                  ))}
                </ul>
              </div>
              <div className="flex w-full flex-col space-y-3 sm:flex-row sm:gap-3 sm:space-y-0 md:w-fit">
                <Button asChild variant="outline" size="sm" className={cn(isScrolled && "lg:hidden")}>
                  <a href="https://github.com/zay168/ossify">
                    <span>GitHub</span>
                  </a>
                </Button>
                <Button asChild size="sm" className={cn(isScrolled && "lg:hidden")}>
                  <a href="#install">
                    <span>Install</span>
                  </a>
                </Button>
                <Button asChild size="sm" className={cn(isScrolled ? "lg:inline-flex" : "hidden")}>
                  <a href="#install">
                    <span>Get Started</span>
                  </a>
                </Button>
              </div>
            </div>
          </div>
        </div>
      </nav>
    </header>
  );
};

const Logo = ({ className }: { className?: string }) => {
  return (
    <div className={cn("inline-flex items-center gap-3", className)}>
      <div className="grid grid-cols-2 gap-1 rounded-xl border border-white/10 bg-white/[0.03] p-2">
        <span className="h-2.5 w-2.5 rounded-[3px] bg-[linear-gradient(135deg,#8be0ff,#f4bf72)]" />
        <span className="h-2.5 w-2.5 rounded-[3px] bg-[linear-gradient(135deg,#8be0ff,#f4bf72)]" />
        <span className="h-2.5 w-2.5 rounded-[3px] bg-[linear-gradient(135deg,#8be0ff,#f4bf72)]" />
        <span className="h-2.5 w-2.5 rounded-[3px] bg-[linear-gradient(135deg,#8be0ff,#f4bf72)]" />
      </div>
      <span className="font-mono text-sm uppercase tracking-[0.22em] text-foreground">ossify</span>
    </div>
  );
};


