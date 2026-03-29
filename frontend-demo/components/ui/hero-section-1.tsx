"use client";

import React from "react";
import {
  ArrowRight,
  ChevronRight,
  FileCheck2,
  GitBranchPlus,
  Menu,
  ShieldCheck,
  Sparkles,
  Wand2,
  X,
} from "lucide-react";

import { AnimatedGroup } from "@/components/ui/animated-group";
import { Button } from "@/components/ui/button";
import { FlickeringFooter } from "@/components/ui/flickering-footer";
import { TextEffect } from "@/components/ui/text-effect";
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
  { name: "Flow", href: "#flow" },
  { name: "GitHub", href: "https://github.com/zay168/ossify" },
];

const trustSignals = [
  { label: "README, metadata, CI, security, release", icon: FileCheck2 },
  { label: "Interactive audit and fix plan", icon: Wand2 },
  { label: "Safe GitHub-aware scaffolding", icon: GitBranchPlus },
  { label: "Signals that make a repo safer to adopt", icon: ShieldCheck },
];

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
            <AnimatedGroup
              variants={{
                container: {
                  visible: {
                    transition: {
                      delayChildren: 1,
                    },
                  },
                },
                item: {
                  hidden: {
                    opacity: 0,
                    y: 20,
                  },
                  visible: {
                    opacity: 1,
                    y: 0,
                    transition: {
                      type: "spring" as const,
                      bounce: 0.3,
                      duration: 2,
                    },
                  },
                },
              }}
              className="absolute inset-0 -z-20"
            >
              <img
                src="/hero-background.png"
                alt="Workspace background"
                className="absolute inset-x-0 top-56 hidden opacity-25 saturate-0 lg:top-24 lg:block"
                width="1800"
                height="1200"
              />
            </AnimatedGroup>

            <div
              aria-hidden
              className="absolute inset-0 -z-10 h-full w-full bg-[radial-gradient(125%_125%_at_50%_100%,transparent_0%,hsl(var(--background))_75%)]"
            />

            <div className="mx-auto max-w-7xl px-6">
              <div className="text-center">
                <AnimatedGroup variants={transitionVariants}>
                  <a
                    href="#install"
                    className="group mx-auto flex w-fit items-center gap-4 rounded-full border border-white/10 bg-muted p-1 pl-4 shadow-md shadow-black/10 transition-all duration-300 hover:bg-background"
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

                  <TextEffect
                    as="h1"
                    preset="blur"
                    per="word"
                    className="mx-auto mt-8 max-w-5xl text-balance font-serif text-6xl tracking-[-0.06em] md:text-7xl lg:mt-16 xl:text-[5.25rem]"
                  >
                    Turn rough repos into projects people trust on sight.
                  </TextEffect>

                  <TextEffect
                    as="p"
                    preset="fade"
                    per="word"
                    delay={0.4}
                    className="mx-auto mt-8 max-w-2xl text-balance text-lg text-muted-foreground"
                  >
                    ossify audits the trust layer around your codebase: docs, metadata,
                    contributor signals, CI, release surface, and the exact gaps that
                    keep a repo from feeling ready.
                  </TextEffect>
                </AnimatedGroup>

                <AnimatedGroup
                  variants={{
                    container: {
                      visible: {
                        transition: {
                          staggerChildren: 0.05,
                          delayChildren: 0.75,
                        },
                      },
                    },
                    ...transitionVariants,
                  }}
                  className="mt-12 flex flex-col items-center justify-center gap-2 md:flex-row"
                >
                  <div className="rounded-[14px] border border-white/10 bg-foreground/10 p-0.5">
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
                </AnimatedGroup>
              </div>
            </div>

            <AnimatedGroup
              variants={{
                container: {
                  visible: {
                    transition: {
                      staggerChildren: 0.05,
                      delayChildren: 0.75,
                    },
                  },
                },
                ...transitionVariants,
              }}
            >
              <div className="relative mt-8 overflow-hidden px-2 sm:mt-12 md:mt-20">
                <div
                  aria-hidden
                  className="absolute inset-0 z-10 bg-gradient-to-b from-transparent from-35% to-background"
                />
                <div className="relative mx-auto max-w-6xl overflow-hidden rounded-[1.75rem] border border-white/10 bg-background/90 p-4 shadow-[0_30px_80px_rgba(0,0,0,0.35)] ring-1 ring-white/10">
                  <div className="relative overflow-hidden rounded-[1.35rem] border border-white/10 bg-[linear-gradient(180deg,rgba(10,18,32,0.98),rgba(8,14,24,0.98))] p-6">
                    <div className="absolute inset-0 bg-[radial-gradient(circle_at_top_left,rgba(125,211,252,0.12),transparent_30%),radial-gradient(circle_at_bottom_right,rgba(251,191,36,0.12),transparent_22%)]" />
                    <div className="relative grid gap-6 lg:grid-cols-[1.15fr_0.85fr]">
                      <div className="space-y-4">
                        <div className="inline-flex items-center gap-2 rounded-full border border-white/10 bg-white/5 px-3 py-1 text-xs uppercase tracking-[0.28em] text-primary">
                          <Sparkles className="size-3.5" />
                          Maintainer-grade output
                        </div>
                        <div className="rounded-2xl border border-white/10 bg-black/25 p-5">
                          <div className="mb-4 flex items-center justify-between text-xs uppercase tracking-[0.18em] text-muted-foreground">
                            <span>OSSIFY PLAN</span>
                            <span>safe preview</span>
                          </div>
                          <div className="flex items-end gap-4">
                            <div>
                              <p className="text-xs uppercase tracking-[0.18em] text-muted-foreground">
                                Current
                              </p>
                              <strong className="text-5xl tracking-[-0.06em]">63</strong>
                            </div>
                            <ArrowRight className="mb-2 size-5 text-[#f4bf72]" />
                            <div>
                              <p className="text-xs uppercase tracking-[0.18em] text-muted-foreground">
                                Estimated
                              </p>
                              <strong className="text-5xl tracking-[-0.06em] text-primary">
                                91
                              </strong>
                            </div>
                          </div>
                          <div className="mt-5 h-2 rounded-full bg-white/8">
                            <div className="h-full w-[78%] rounded-full bg-[linear-gradient(90deg,#7dd3fc,#f4bf72)]" />
                          </div>
                        </div>
                      </div>

                      <div className="grid gap-3">
                        {[
                          ["CREATED", "README.md"],
                          ["CREATED", ".github/workflows/ci.yml"],
                          ["SKIPPED", ".github/FUNDING.yml"],
                          ["MANUAL", "README examples still need review"],
                        ].map(([status, label]) => (
                          <div
                            key={label}
                            className="flex items-center justify-between gap-4 rounded-2xl border border-white/10 bg-white/[0.03] px-4 py-3"
                          >
                            <span
                              className={cn(
                                "inline-flex min-w-24 items-center justify-center rounded-full px-3 py-1 text-[11px] uppercase tracking-[0.18em]",
                                status === "CREATED" && "bg-emerald-400/14 text-emerald-200",
                                status === "SKIPPED" && "bg-amber-300/12 text-amber-200",
                                status === "MANUAL" && "bg-sky-300/12 text-sky-200",
                              )}
                            >
                              {status}
                            </span>
                            <span className="text-right text-sm text-muted-foreground">
                              {label}
                            </span>
                          </div>
                        ))}
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </AnimatedGroup>
          </div>
        </section>

        <section id="signals" className="bg-background pb-16 pt-16 md:pb-32">
          <div className="group relative m-auto max-w-5xl px-6">
            <div className="absolute inset-0 z-10 flex scale-95 items-center justify-center opacity-0 transition-all duration-500 group-hover:scale-100 group-hover:opacity-100">
              <a href="#flow" className="block text-sm transition-opacity duration-150 hover:opacity-75">
                <span>See the maintainer workflow</span>
                <ChevronRight className="ml-1 inline-block size-3" />
              </a>
            </div>
            <div className="mx-auto mt-12 grid max-w-4xl gap-4 transition-all duration-500 group-hover:opacity-45 sm:grid-cols-2">
              {trustSignals.map((signal) => {
                const Icon = signal.icon;
                return (
                  <div
                    key={signal.label}
                    className="flex items-center gap-3 rounded-2xl border border-white/10 bg-white/[0.03] px-4 py-4"
                  >
                    <div className="rounded-xl border border-white/10 bg-white/[0.04] p-2.5">
                      <Icon className="size-4 text-primary" />
                    </div>
                    <p className="text-sm text-muted-foreground">{signal.label}</p>
                  </div>
                );
              })}
            </div>
          </div>
        </section>

        <section id="install" className="mx-auto max-w-6xl px-6 pb-24">
          <div id="flow" className="grid gap-6 lg:grid-cols-[0.85fr_1.15fr]">
            <div className="rounded-[1.75rem] border border-white/10 bg-muted/30 p-8">
              <p className="text-xs uppercase tracking-[0.28em] text-primary">
                One-line install
              </p>
              <h2 className="mt-4 max-w-sm font-serif text-4xl tracking-[-0.05em]">
                Native bootstrap scripts for the latest release.
              </h2>
              <p className="mt-4 max-w-md text-muted-foreground">
                PowerShell on Windows, shell on macOS and Linux, and a predictable
                binary path. The landing is now wired around the actual install flow.
              </p>
            </div>
            <div className="grid gap-4">
              <div className="rounded-[1.5rem] border border-white/10 bg-background/80 p-5">
                <p className="text-xs uppercase tracking-[0.24em] text-muted-foreground">
                  Windows PowerShell
                </p>
                <code className="mt-3 block overflow-x-auto rounded-2xl border border-white/10 bg-black/30 px-4 py-4 text-sm text-primary">
                  irm https://zay168.github.io/ossify/install.ps1 | iex
                </code>
              </div>
              <div className="rounded-[1.5rem] border border-white/10 bg-background/80 p-5">
                <p className="text-xs uppercase tracking-[0.24em] text-muted-foreground">
                  macOS / Linux
                </p>
                <code className="mt-3 block overflow-x-auto rounded-2xl border border-white/10 bg-black/30 px-4 py-4 text-sm text-primary">
                  curl -fsSL https://zay168.github.io/ossify/install.sh | sh
                </code>
              </div>
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

    window.addEventListener("scroll", handleScroll);
    return () => window.removeEventListener("scroll", handleScroll);
  }, []);

  return (
    <header>
      <nav data-state={menuState ? "active" : "inactive"} className="group fixed z-20 w-full px-2">
        <div
          className={cn(
            "mx-auto mt-2 max-w-6xl px-4 transition-all duration-300 lg:px-8",
            isScrolled && "max-w-4xl rounded-[1.3rem] border border-white/10 bg-background/65 backdrop-blur-xl",
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

            <div className="mb-6 hidden w-full flex-wrap items-center justify-end space-y-8 rounded-[1.75rem] border border-white/10 bg-background p-6 shadow-2xl shadow-zinc-950/20 md:flex-nowrap lg:m-0 lg:flex lg:w-fit lg:gap-3 lg:space-y-0 lg:border-transparent lg:bg-transparent lg:p-0 lg:shadow-none group-data-[state=active]:block lg:group-data-[state=active]:flex">
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
