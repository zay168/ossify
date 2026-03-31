import React, { type ComponentPropsWithoutRef, useMemo, useRef } from "react";

import { cn } from "@/lib/utils";

interface MarqueeProps extends ComponentPropsWithoutRef<"div"> {
  className?: string;
  reverse?: boolean;
  pauseOnHover?: boolean;
  children: React.ReactNode;
  vertical?: boolean;
  repeat?: number;
  autoFill?: boolean;
  ariaLabel?: string;
  ariaLive?: "off" | "polite" | "assertive";
  ariaRole?: React.AriaRole;
}

export function Marquee({
  className,
  reverse = false,
  pauseOnHover = false,
  children,
  vertical = false,
  repeat = 4,
  autoFill = false,
  ariaLabel,
  ariaLive = "off",
  ariaRole = "region",
  ...props
}: MarqueeProps) {
  const marqueeRef = useRef<HTMLDivElement>(null);

  const instances = useMemo(() => {
    const safeRepeat = autoFill ? Math.max(repeat, 6) : repeat;

    return Array.from({ length: safeRepeat }, (_, index) => (
      <div
        key={index}
        className={cn(
          "flex shrink-0 justify-around [gap:var(--gap)]",
          !vertical && "animate-marquee flex-row",
          vertical && "animate-marquee-vertical flex-col",
          pauseOnHover && "group-hover:[animation-play-state:paused]",
          reverse && "[animation-direction:reverse]",
        )}
      >
        {children}
      </div>
    ));
  }, [autoFill, children, pauseOnHover, repeat, reverse, vertical]);

  return (
    <div
      {...props}
      ref={marqueeRef}
      data-slot="marquee"
      className={cn(
        "group flex overflow-hidden p-2 [--duration:40s] [--gap:1rem] [gap:var(--gap)]",
        vertical ? "flex-col" : "flex-row",
        className,
      )}
      aria-label={ariaLabel}
      aria-live={ariaLive}
      role={ariaRole}
      tabIndex={0}
    >
      {instances}
    </div>
  );
}
