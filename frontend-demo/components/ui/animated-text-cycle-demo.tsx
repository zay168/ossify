import AnimatedTextCycle from "@/components/ui/animated-text-cycle";

export function AnimatedTextCycleDemo() {
  return (
    <p className="inline-flex max-w-full items-baseline gap-2 whitespace-nowrap text-[clamp(0.95rem,1.6vw,1.75rem)] font-light tracking-[-0.03em] text-muted-foreground">
      <span className="opacity-85">Your</span>
      <span className="inline-flex items-baseline">
        <AnimatedTextCycle
          words={[
            "repo",
            "readme",
            "release flow",
            "metadata",
            "CI surface",
            "contributors",
            "workflow",
            "adoption story",
          ]}
          interval={3000}
          className="font-semibold text-foreground"
        />
      </span>
      <span className="opacity-85">deserves better trust signals</span>
    </p>
  );
}
