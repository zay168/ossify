"use client";

import { ChevronRightIcon } from "@radix-ui/react-icons";
import {
  BadgeCheck,
  Binary,
  ShieldCheck,
  Sparkles,
  WandSparkles,
} from "lucide-react";
import { ClassValue, clsx } from "clsx";
import * as Color from "color-bits";
import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export const getRGBA = (
  cssColor: React.CSSProperties["color"],
  fallback: string = "rgba(180, 180, 180)",
): string => {
  if (typeof window === "undefined") return fallback;
  if (!cssColor) return fallback;

  try {
    if (typeof cssColor === "string" && cssColor.startsWith("var(")) {
      const element = document.createElement("div");
      element.style.color = cssColor;
      document.body.appendChild(element);
      const computedColor = window.getComputedStyle(element).color;
      document.body.removeChild(element);
      return Color.formatRGBA(Color.parse(computedColor));
    }

    return Color.formatRGBA(Color.parse(cssColor));
  } catch (error) {
    console.error("Color parsing failed:", error);
    return fallback;
  }
};

export const colorWithOpacity = (color: string, opacity: number): string => {
  if (!color.startsWith("rgb")) return color;
  return Color.formatRGBA(Color.alpha(Color.parse(color), opacity));
};

export const focusInput = [
  "focus:ring-2",
  "focus:ring-blue-200 focus:dark:ring-blue-700/30",
  "focus:border-blue-500 focus:dark:border-blue-700",
];

export const focusRing = [
  "outline outline-offset-2 outline-0 focus-visible:outline-2",
  "outline-blue-500 dark:outline-blue-500",
];

export const hasErrorInput = [
  "ring-2",
  "border-red-500 dark:border-red-700",
  "ring-red-200 dark:ring-red-700/30",
];

const ComplianceBadge = ({
  icon: Icon,
  label,
}: {
  icon: React.ComponentType<{ className?: string }>;
  label: string;
}) => (
  <div className="inline-flex items-center gap-2 rounded-full border border-white/10 bg-white/[0.04] px-3 py-2 text-sm text-muted-foreground">
    <div className="rounded-full border border-white/10 bg-white/[0.06] p-1.5">
      <Icon className="size-4 text-primary" />
    </div>
    <span>{label}</span>
  </div>
);

export const Icons = {
  logo: ({ className }: { className?: string }) => (
    <div className={cn("inline-flex items-center gap-3", className)}>
      <div className="grid grid-cols-2 gap-1 rounded-xl border border-white/10 bg-white/[0.03] p-2">
        <span className="h-2.5 w-2.5 rounded-[3px] bg-[linear-gradient(135deg,#8be0ff,#f4bf72)]" />
        <span className="h-2.5 w-2.5 rounded-[3px] bg-[linear-gradient(135deg,#8be0ff,#f4bf72)]" />
        <span className="h-2.5 w-2.5 rounded-[3px] bg-[linear-gradient(135deg,#8be0ff,#f4bf72)]" />
        <span className="h-2.5 w-2.5 rounded-[3px] bg-[linear-gradient(135deg,#8be0ff,#f4bf72)]" />
      </div>
      <span className="font-mono text-sm uppercase tracking-[0.22em] text-foreground">
        ossify
      </span>
    </div>
  ),
  soc2: ({ className }: { className?: string }) => (
    <ComplianceBadge icon={ShieldCheck} label="SOC 2" />
  ),
  soc2Dark: ({ className }: { className?: string }) => (
    <ComplianceBadge icon={ShieldCheck} label="SOC 2" />
  ),
  hipaa: ({ className }: { className?: string }) => (
    <ComplianceBadge icon={BadgeCheck} label="Security" />
  ),
  hipaaDark: ({ className }: { className?: string }) => (
    <ComplianceBadge icon={BadgeCheck} label="Security" />
  ),
  gdpr: ({ className }: { className?: string }) => (
    <ComplianceBadge icon={Binary} label="Offline" />
  ),
  gdprDark: ({ className }: { className?: string }) => (
    <ComplianceBadge icon={Binary} label="Offline" />
  ),
};

interface FlickeringGridProps extends React.HTMLAttributes<HTMLDivElement> {
  squareSize?: number;
  gridGap?: number;
  flickerChance?: number;
  color?: string;
  width?: number;
  height?: number;
  className?: string;
  maxOpacity?: number;
  text?: string;
  textColor?: string;
  fontSize?: number;
  fontWeight?: number | string;
}

export const FlickeringGrid: React.FC<FlickeringGridProps> = ({
  squareSize = 3,
  gridGap = 3,
  flickerChance = 0.2,
  color = "#B4B4B4",
  width,
  height,
  className,
  maxOpacity = 0.15,
  text = "",
  fontSize = 140,
  fontWeight = 600,
  ...props
}) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [isInView, setIsInView] = useState(false);
  const [canvasSize, setCanvasSize] = useState({ width: 0, height: 0 });
  const prefersReducedMotion = useMemo(
    () => typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches,
    [],
  );

  const memoizedColor = useMemo(() => getRGBA(color), [color]);

  const drawGrid = useCallback(
    (
      ctx: CanvasRenderingContext2D,
      widthPx: number,
      heightPx: number,
      cols: number,
      rows: number,
      squares: Float32Array,
      mask: Uint8Array,
    ) => {
      ctx.clearRect(0, 0, widthPx, heightPx);

      for (let col = 0; col < cols; col++) {
        for (let row = 0; row < rows; row++) {
          const index = col * rows + row;
          const x = col * (squareSize + gridGap);
          const y = row * (squareSize + gridGap);
          const opacity = squares[index];
          const hasText = mask[index] === 1;
          const finalOpacity = hasText ? Math.min(1, opacity * 3 + 0.4) : opacity;

          ctx.fillStyle = colorWithOpacity(memoizedColor, finalOpacity);
          ctx.fillRect(x, y, squareSize, squareSize);
        }
      }
    },
    [gridGap, memoizedColor, squareSize],
  );

  const setupCanvas = useCallback(
    (canvas: HTMLCanvasElement, canvasWidth: number, canvasHeight: number) => {
      const dpr = Math.min(window.devicePixelRatio || 1, 1.5);
      canvas.width = Math.floor(canvasWidth * dpr);
      canvas.height = Math.floor(canvasHeight * dpr);
      canvas.style.width = `${canvasWidth}px`;
      canvas.style.height = `${canvasHeight}px`;

      const cols = Math.ceil(canvasWidth / (squareSize + gridGap));
      const rows = Math.ceil(canvasHeight / (squareSize + gridGap));
      const squares = new Float32Array(cols * rows);
      const mask = new Uint8Array(cols * rows);

      for (let i = 0; i < squares.length; i++) {
        squares[i] = Math.random() * maxOpacity;
      }

      if (text) {
        const maskCanvas = document.createElement("canvas");
        maskCanvas.width = canvasWidth;
        maskCanvas.height = canvasHeight;
        const maskCtx = maskCanvas.getContext("2d", { willReadFrequently: true });

        if (maskCtx) {
          maskCtx.fillStyle = "white";
          maskCtx.font = `${fontWeight} ${fontSize}px "IBM Plex Sans", -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif`;
          maskCtx.textAlign = "center";
          maskCtx.textBaseline = "middle";
          maskCtx.fillText(text, canvasWidth / 2, canvasHeight / 2);

          for (let col = 0; col < cols; col++) {
            for (let row = 0; row < rows; row++) {
              const x = col * (squareSize + gridGap);
              const y = row * (squareSize + gridGap);
              const pixel = maskCtx.getImageData(x, y, squareSize, squareSize).data;
              mask[col * rows + row] = pixel.some((value, index) => index % 4 === 0 && value > 0)
                ? 1
                : 0;
            }
          }
        }
      }

      const ctx = canvas.getContext("2d");
      if (ctx) {
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      }

      return { cols, rows, squares, mask };
    },
    [fontSize, fontWeight, gridGap, maxOpacity, squareSize, text],
  );

  const updateSquares = useCallback(
    (squares: Float32Array, deltaTime: number) => {
      if (prefersReducedMotion) return;

      for (let i = 0; i < squares.length; i++) {
        if (Math.random() < flickerChance * deltaTime) {
          squares[i] = Math.random() * maxOpacity;
        }
      }
    },
    [flickerChance, maxOpacity, prefersReducedMotion],
  );

  useEffect(() => {
    const canvas = canvasRef.current;
    const container = containerRef.current;
    if (!canvas || !container) return;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    let animationFrameId = 0;
    let gridParams = setupCanvas(canvas, width || container.clientWidth, height || container.clientHeight);

    const updateCanvasSize = () => {
      const nextWidth = width || container.clientWidth;
      const nextHeight = height || container.clientHeight;
      setCanvasSize({ width: nextWidth, height: nextHeight });
      gridParams = setupCanvas(canvas, nextWidth, nextHeight);
    };

    updateCanvasSize();

    let lastTime = 0;
    let lastDrawTime = 0;
    const animate = (time: number) => {
      if (!isInView) return;
      const deltaTime = (time - lastTime) / 1000;
      lastTime = time;
      const frameInterval = prefersReducedMotion ? 1000 / 12 : 1000 / 30;

      if (time - lastDrawTime < frameInterval) {
        animationFrameId = requestAnimationFrame(animate);
        return;
      }
      lastDrawTime = time;

      updateSquares(gridParams.squares, deltaTime);
      drawGrid(
        ctx,
        canvas.width / Math.min(window.devicePixelRatio || 1, 1.5),
        canvas.height / Math.min(window.devicePixelRatio || 1, 1.5),
        gridParams.cols,
        gridParams.rows,
        gridParams.squares,
        gridParams.mask,
      );

      animationFrameId = requestAnimationFrame(animate);
    };

    const resizeObserver = new ResizeObserver(updateCanvasSize);
    resizeObserver.observe(container);

    const intersectionObserver = new IntersectionObserver(
      ([entry]) => setIsInView(entry.isIntersecting),
      { threshold: 0 },
    );
    intersectionObserver.observe(canvas);

    if (isInView) {
      animationFrameId = requestAnimationFrame(animate);
    }

    return () => {
      cancelAnimationFrame(animationFrameId);
      resizeObserver.disconnect();
      intersectionObserver.disconnect();
    };
  }, [drawGrid, height, isInView, prefersReducedMotion, setupCanvas, updateSquares, width]);

  return (
    <div ref={containerRef} className={cn("h-full w-full", className)} {...props}>
      <canvas
        ref={canvasRef}
        className="pointer-events-none"
        style={{
          width: canvasSize.width,
          height: canvasSize.height,
        }}
      />
    </div>
  );
};

export function useMediaQuery(query: string) {
  const [value, setValue] = useState(false);

  useEffect(() => {
    function checkQuery() {
      const result = window.matchMedia(query);
      setValue(result.matches);
    }

    checkQuery();
    window.addEventListener("resize", checkQuery);

    const mediaQuery = window.matchMedia(query);
    mediaQuery.addEventListener("change", checkQuery);

    return () => {
      window.removeEventListener("resize", checkQuery);
      mediaQuery.removeEventListener("change", checkQuery);
    };
  }, [query]);

  return value;
}

export const Highlight = ({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) => {
  return (
    <span className={cn("p-1 py-0.5 font-medium text-secondary dark:font-semibold", className)}>
      {children}
    </span>
  );
};

export const BLUR_FADE_DELAY = 0.15;

export const siteConfig = {
  hero: {
    badge: "Launch-ready trust signals",
    title: "Turn rough repos into projects people trust",
    description:
      "Audit maintainers signals, preview safe fixes, and scaffold the missing trust layer around your repository without losing control.",
    cta: {
      primary: {
        text: "Install ossify",
        href: "#install",
      },
      secondary: {
        text: "GitHub",
        href: "https://github.com/zay168/ossify",
      },
    },
  },
  footerLinks: [
    {
      title: "Product",
      links: [
        { id: 1, title: "Audit", url: "#signals" },
        { id: 2, title: "Fix Plan", url: "#flow" },
        { id: 3, title: "Install", url: "#install" },
        { id: 4, title: "Prompt", url: "https://github.com/zay168/ossify" },
      ],
    },
    {
      title: "Resources",
      links: [
        { id: 5, title: "GitHub", url: "https://github.com/zay168/ossify" },
        { id: 6, title: "Releases", url: "https://github.com/zay168/ossify/releases" },
        { id: 7, title: "License", url: "https://github.com/zay168/ossify/blob/main/LICENSE" },
        { id: 8, title: "Security", url: "https://github.com/zay168/ossify/blob/main/SECURITY.md" },
      ],
    },
    {
      title: "Installers",
      links: [
        { id: 9, title: "install.ps1", url: "https://zay168.github.io/ossify/install.ps1" },
        { id: 10, title: "install.sh", url: "https://zay168.github.io/ossify/install.sh" },
        { id: 11, title: "GitHub Pages", url: "https://zay168.github.io/ossify/" },
        { id: 12, title: "Releases API", url: "https://github.com/zay168/ossify/releases/latest" },
      ],
    },
  ],
};

export type SiteConfig = typeof siteConfig;

export function FlickeringFooter() {
  const tablet = useMediaQuery("(max-width: 1024px)");

  return (
    <footer id="footer" className="w-full pb-0">
      <div className="flex flex-col p-10 md:flex-row md:items-center md:justify-between">
        <div className="mx-0 flex max-w-xs flex-col items-start justify-start gap-y-5">
          <a href="#top" className="flex items-center gap-2">
            <Icons.logo className="size-8" />
          </a>
          <p className="font-medium tracking-tight text-muted-foreground">
            {siteConfig.hero.description}
          </p>
          <div className="flex items-center gap-2">
            <Icons.soc2 className="size-12" />
            <Icons.hipaa className="size-12" />
            <Icons.gdpr className="size-12" />
          </div>
        </div>

        <div className="pt-5 md:w-1/2">
          <div className="flex flex-col items-start justify-start gap-y-5 md:flex-row md:items-center md:justify-between lg:pl-10">
            {siteConfig.footerLinks.map((column) => (
              <ul key={column.title} className="flex flex-col gap-y-2">
                <li className="mb-2 text-sm font-semibold text-primary">{column.title}</li>
                {column.links.map((link) => (
                  <li
                    key={link.id}
                    className="group inline-flex cursor-pointer items-center justify-start gap-1 text-[15px]/snug text-muted-foreground"
                  >
                    <a href={link.url}>{link.title}</a>
                    <div className="flex size-4 translate-x-0 transform items-center justify-center rounded border border-border opacity-0 transition-all duration-300 ease-out group-hover:translate-x-1 group-hover:opacity-100">
                      <ChevronRightIcon className="h-4 w-4" />
                    </div>
                  </li>
                ))}
              </ul>
            ))}
          </div>
        </div>
      </div>

      <div className="relative z-0 mt-24 h-48 w-full md:h-64">
        <div className="absolute inset-0 z-10 bg-gradient-to-t from-transparent from-40% to-background" />
        <div className="absolute inset-0 mx-6">
          <FlickeringGrid
            text={tablet ? "ossify" : "trust ships here"}
            fontSize={tablet ? 70 : 94}
            className="h-full w-full"
            squareSize={2}
            gridGap={tablet ? 2 : 3}
            color="#6B7280"
            maxOpacity={0.3}
            flickerChance={0.1}
          />
        </div>
      </div>
    </footer>
  );
}
