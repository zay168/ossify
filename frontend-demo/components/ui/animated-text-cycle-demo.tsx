import AnimatedTextCycle from "@/components/ui/animated-text-cycle";

export function AnimatedTextCycleDemo() {
  return (
    <p className="flex max-w-[52rem] flex-wrap items-baseline justify-center gap-x-3 gap-y-2 px-4 text-center text-[clamp(1rem,1.7vw,1.8rem)] font-light leading-[1.45] tracking-[-0.03em] text-muted-foreground md:gap-x-4">
      <span className="pr-1 opacity-80">Your</span>
      <span className="inline-flex items-baseline px-1">
        <AnimatedTextCycle
          words={[
            "repo",
            "README",
            "release notes",
            "dependency policy",
            "CI surface",
            "contributors",
            "workflow permissions",
            "adoption story",
          ]}
          interval={3000}
          className="font-semibold text-foreground"
        />
      </span>
      <span className="pl-1 opacity-80">deserves better trust signals</span>
    </p>
  );
}
