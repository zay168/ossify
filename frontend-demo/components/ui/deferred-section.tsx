"use client";

import * as React from "react";

type DeferredSectionProps = {
  children: React.ReactNode;
  className?: string;
  minHeight?: number;
  rootMargin?: string;
  placeholder?: React.ReactNode;
  idleTimeoutMs?: number;
};

type IdleCapableWindow = Window &
  typeof globalThis & {
    requestIdleCallback?: (
      callback: () => void,
      options?: {
        timeout?: number;
      },
    ) => number;
    cancelIdleCallback?: (handle: number) => void;
  };

export function DeferredSection({
  children,
  className,
  minHeight,
  rootMargin = "280px 0px",
  placeholder = null,
  idleTimeoutMs = 1800,
}: DeferredSectionProps) {
  const containerRef = React.useRef<HTMLDivElement>(null);
  const [shouldMount, setShouldMount] = React.useState(false);

  React.useEffect(() => {
    if (shouldMount) return;

    const node = containerRef.current;
    if (!node) return;

    const enable = () => setShouldMount(true);
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) {
          enable();
          observer.disconnect();
        }
      },
      { rootMargin },
    );

    observer.observe(node);

    const idleWindow = window as IdleCapableWindow;
    let cleanupIdle: (() => void) | undefined;

    if (idleWindow.requestIdleCallback) {
      const idleId = idleWindow.requestIdleCallback(enable, { timeout: idleTimeoutMs });
      cleanupIdle = () => idleWindow.cancelIdleCallback?.(idleId);
    } else {
      const timeoutId = window.setTimeout(enable, idleTimeoutMs);
      cleanupIdle = () => window.clearTimeout(timeoutId);
    }

    return () => {
      observer.disconnect();
      cleanupIdle?.();
    };
  }, [idleTimeoutMs, rootMargin, shouldMount]);

  return (
    <div ref={containerRef} className={className} style={minHeight ? { minHeight } : undefined}>
      {shouldMount ? children : placeholder}
    </div>
  );
}
