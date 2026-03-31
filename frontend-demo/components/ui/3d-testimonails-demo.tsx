"use client";

import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { Marquee } from "@/components/ui/3d-testimonails";
import { Card, CardContent } from "@/components/ui/card";
import { cn } from "@/lib/utils";

const testimonialColumns = [
  {
    name: "Docs Lead",
    username: "@first-run",
    body: "The install path finally reads like a project I can recommend without adding caveats.",
    img: "https://images.unsplash.com/photo-1494790108377-be9c29b29330?auto=format&fit=crop&w=160&q=80",
    country: "Docs",
  },
  {
    name: "Security Lead",
    username: "@least-privilege",
    body: "Permissions, workflows, metadata and release hygiene all surface in one pass instead of four separate reviews.",
    img: "https://images.unsplash.com/photo-1500648767791-00dcc994a43e?auto=format&fit=crop&w=160&q=80",
    country: "CI",
  },
  {
    name: "Maintainer Note",
    username: "@contrib-friendly",
    body: "It helps close the subtle gaps that make healthy repos feel instantly more trustworthy to outside contributors.",
    img: "https://images.unsplash.com/photo-1438761681033-6461ffad8d80?auto=format&fit=crop&w=160&q=80",
    country: "Trust",
  },
  {
    name: "Release Owner",
    username: "@ship-clean",
    body: "The release surface feels less improvised when the changelog, CI and install story line up on the same page.",
    img: "https://images.unsplash.com/photo-1506794778202-cad84cf45f1d?auto=format&fit=crop&w=160&q=80",
    country: "Ship",
  },
  {
    name: "OSS Lead",
    username: "@repo-trust",
    body: "The repo stops feeling like a promising prototype and starts feeling ready to adopt.",
    img: "https://images.unsplash.com/photo-1488426862026-3ee34a7d66df?auto=format&fit=crop&w=160&q=80",
    country: "Adopt",
  },
  {
    name: "DX Pass",
    username: "@zero-friction",
    body: "Clear docs plus a visible fix plan make the project easier to understand before anyone reads the code deeply.",
    img: "https://images.unsplash.com/photo-1504593811423-6dd665756598?auto=format&fit=crop&w=160&q=80",
    country: "DX",
  },
  {
    name: "Contributor View",
    username: "@first-pr",
    body: "I know what to trust, where to start, and which signals are intentionally maintained.",
    img: "https://images.unsplash.com/photo-1517841905240-472988babdf9?auto=format&fit=crop&w=160&q=80",
    country: "Onboard",
  },
  {
    name: "Audit Trail",
    username: "@ship-safe",
    body: "The strongest part is how it turns repo quality from vague taste into concrete, fixable signals.",
    img: "https://images.unsplash.com/photo-1508214751196-bcfd4ca60f91?auto=format&fit=crop&w=160&q=80",
    country: "Review",
  },
];

function TestimonialCard({
  img,
  name,
  username,
  body,
  country,
}: (typeof testimonialColumns)[number]) {
  return (
    <Card className="w-[18.75rem] rounded-[1.4rem] border-white/10 bg-[linear-gradient(180deg,rgba(14,21,37,0.96),rgba(9,14,25,0.92))] shadow-[0_18px_45px_rgba(0,0,0,0.25)] backdrop-blur">
      <CardContent className="p-4 pt-4">
        <div className="flex items-center gap-3">
          <Avatar className="size-10 border border-white/10">
            <AvatarImage src={img} alt={name} />
            <AvatarFallback className="bg-white/5 text-xs text-foreground">
              {name.slice(0, 2).toUpperCase()}
            </AvatarFallback>
          </Avatar>
          <div className="min-w-0 flex-1">
            <figcaption className="flex items-center justify-between gap-2 text-sm font-medium text-foreground">
              <span className="truncate pr-2">{name}</span>
              <span className="rounded-full border border-primary/20 bg-primary/10 px-2 py-0.5 text-[10px] uppercase tracking-[0.18em] text-primary">
                {country}
              </span>
            </figcaption>
            <p className="text-xs text-muted-foreground">{username}</p>
          </div>
        </div>
        <blockquote className="mt-4 text-[13px] leading-6 text-secondary-foreground/88">
          {body}
        </blockquote>
      </CardContent>
    </Card>
  );
}

export function ThreeDTestimonialsDemo({ className }: { className?: string }) {
  const columnClasses = "w-[19rem] [--duration:34s] md:[--duration:38s]";

  return (
    <div
      className={cn(
        "relative flex h-[32rem] w-full items-center justify-center overflow-hidden rounded-[2rem] border border-white/10 bg-[linear-gradient(180deg,rgba(15,21,37,0.9),rgba(8,13,24,0.95))] shadow-[0_35px_90px_rgba(0,0,0,0.34)] [perspective:1200px] md:h-[34rem]",
        className,
      )}
    >
      <div className="absolute inset-0 bg-[radial-gradient(circle_at_top,rgba(125,211,252,0.14),transparent_28%),radial-gradient(circle_at_bottom_right,rgba(244,191,114,0.12),transparent_22%)]" />
      <div className="absolute left-6 top-6 rounded-full border border-white/10 bg-white/[0.04] px-3 py-1 text-[11px] uppercase tracking-[0.24em] text-primary">
        Adoption signals in motion
      </div>

      <div
        className="relative flex w-full items-center justify-center gap-2 px-4 pt-10 md:gap-3"
        style={{
          transform:
            "translateX(-18px) translateY(10px) translateZ(-60px) rotateX(10deg) rotateY(-10deg) rotateZ(6deg)",
        }}
      >
        <Marquee
          vertical
          pauseOnHover
          repeat={3}
          className={cn(columnClasses)}
          ariaLabel="Maintainer proof column one"
        >
          {testimonialColumns.map((review) => (
            <TestimonialCard key={`a-${review.username}`} {...review} />
          ))}
        </Marquee>
        <Marquee
          vertical
          pauseOnHover
          reverse
          repeat={3}
          className={cn(columnClasses)}
          ariaLabel="Maintainer proof column two"
        >
          {testimonialColumns.map((review) => (
            <TestimonialCard key={`b-${review.username}`} {...review} />
          ))}
        </Marquee>
        <Marquee
          vertical
          pauseOnHover
          repeat={3}
          className={cn(columnClasses, "hidden lg:flex")}
          ariaLabel="Maintainer proof column three"
        >
          {testimonialColumns.map((review) => (
            <TestimonialCard key={`c-${review.username}`} {...review} />
          ))}
        </Marquee>
        <Marquee
          vertical
          pauseOnHover
          reverse
          repeat={3}
          className={cn(columnClasses, "hidden 2xl:flex")}
          ariaLabel="Maintainer proof column four"
        >
          {testimonialColumns.map((review) => (
            <TestimonialCard key={`d-${review.username}`} {...review} />
          ))}
        </Marquee>
      </div>

      <div className="pointer-events-none absolute inset-x-0 top-0 h-24 bg-gradient-to-b from-background via-background/70 to-transparent" />
      <div className="pointer-events-none absolute inset-x-0 bottom-0 h-24 bg-gradient-to-t from-background via-background/70 to-transparent" />
      <div className="pointer-events-none absolute inset-y-0 left-0 w-16 bg-gradient-to-r from-background via-background/70 to-transparent md:w-24" />
      <div className="pointer-events-none absolute inset-y-0 right-0 w-16 bg-gradient-to-l from-background via-background/70 to-transparent md:w-24" />
    </div>
  );
}
