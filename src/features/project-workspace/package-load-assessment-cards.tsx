import {
  AlertTriangle,
  CheckCircle2,
  Circle,
  FileCheck2,
  FileText,
  Gauge,
  Hammer,
  Loader2,
  ShieldAlert,
  Target,
  XCircle,
} from "lucide-react";
import type React from "react";

import { Card } from "@/components/ui/card";
import type {
  PackageLoadAssessment,
  PackageLoadAssessmentCommandId,
  PackageLoadAssessmentState,
  PackageLoadAssessmentStep,
} from "@peregrine/desktop-runtime";
import { packageLoadAssessmentCommands } from "@peregrine/desktop-runtime";
import { cn } from "@/lib/utils";

type PackageLoadAssessmentCardsProps = {
  assessment: PackageLoadAssessment | null;
};

const assessmentIconById: Record<
  PackageLoadAssessmentCommandId,
  React.ComponentType<React.SVGProps<SVGSVGElement>>
> = {
  build: Hammer,
  coverage: Gauge,
  formal: FileText,
  fuzzing: Target,
  risk: ShieldAlert,
  tests: FileCheck2,
};

export function PackageLoadAssessmentCards({
  assessment,
}: PackageLoadAssessmentCardsProps) {
  if (!assessment) {
    return null;
  }

  return (
    <section className="grid min-w-0 grid-cols-[repeat(6,minmax(0,1fr))] gap-2">
      {assessment.steps.map((step) => {
        const Icon = assessmentIconById[step.id];

        return (
          <Card
            className={cn(
              "grid h-[82px] min-w-0 grid-rows-[18px_24px_16px] gap-1 overflow-hidden rounded-md px-3 py-2.5 shadow-none",
              step.state === "muted" && "opacity-45",
              step.state === "skipped" && "opacity-60",
              step.state === "running" && "border-primary/45",
              step.state === "success" && "border-emerald-500/30",
              step.state === "attention" && "border-amber-500/35",
              step.state === "error" && "border-red-500/35",
            )}
            key={step.id}
          >
            <div className="grid min-w-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-2">
              <p className="min-w-0 truncate text-[11px] font-medium leading-[18px] text-muted-foreground">
                {step.label}
              </p>
              <Icon
                className={cn(
                  "size-3.5 shrink-0 text-muted-foreground",
                  step.state === "running" && "text-primary",
                  step.state === "success" && "text-emerald-400",
                  step.state === "attention" && "text-amber-400",
                  step.state === "error" && "text-red-400",
                )}
                aria-hidden="true"
              />
            </div>

            <div className="flex min-w-0 items-center gap-1.5">
              <StepStateIcon state={step.state} />
              {visibleStepValue(step) ? (
                <p
                  className={cn(
                    "min-w-0 truncate text-sm font-semibold leading-6",
                    valueToneClass(step.state),
                  )}
                >
                  {visibleStepValue(step)}
                </p>
              ) : null}
            </div>

            <p
              className="truncate text-[10px] leading-4 text-muted-foreground"
              title={step.detail ?? step.caption}
            >
              {step.detail ?? step.caption}
            </p>
          </Card>
        );
      })}
    </section>
  );
}

export function assessmentSidebarItems(assessment: PackageLoadAssessment | null) {
  return (assessment?.steps ?? defaultSidebarAssessmentSteps())
    .filter((step) => step.id !== "risk")
    .map((step) => ({
      badge: sidebarBadge(step),
      icon: assessmentIconById[step.id],
      label: step.label,
      tone: sidebarTone(step.state),
    }));
}

function StepStateIcon({ state }: { state: PackageLoadAssessmentState }) {
  let icon: React.ReactNode;

  if (state === "running") {
    icon = <Loader2 className="size-3.5 animate-spin text-primary" aria-hidden="true" />;
  } else if (state === "success") {
    icon = <CheckCircle2 className="size-3.5 text-emerald-400" aria-hidden="true" />;
  } else if (state === "error") {
    icon = <XCircle className="size-3.5 text-red-400" aria-hidden="true" />;
  } else if (state === "attention") {
    icon = <AlertTriangle className="size-3.5 text-amber-400" aria-hidden="true" />;
  } else {
    icon = <Circle className="size-3.5 text-muted-foreground" aria-hidden="true" />;
  }

  return (
    <span className="grid size-4 shrink-0 place-items-center leading-none" aria-hidden="true">
      {icon}
    </span>
  );
}

function visibleStepValue(step: PackageLoadAssessmentStep) {
  if (step.state === "skipped") {
    return "Skipped";
  }

  if (step.state === "attention") {
    return "Review";
  }

  return null;
}

function valueToneClass(state: PackageLoadAssessmentState) {
  switch (state) {
    case "success":
      return "text-emerald-400";
    case "error":
      return "text-red-400";
    case "running":
      return "text-primary";
    case "attention":
      return "text-amber-400";
    case "skipped":
    case "idle":
    case "muted":
      return "text-foreground";
  }
}

function sidebarBadge(step: PackageLoadAssessmentStep) {
  if (step.state === "success") {
    return "check";
  }

  if (step.state === "error") {
    return "x";
  }

  if (step.state === "attention") {
    return "Review";
  }

  if (step.state === "running") {
    return "spinner";
  }

  if (step.state === "idle") {
    return "circle";
  }

  return step.state === "skipped" ? "Skipped" : undefined;
}

function sidebarTone(state: PackageLoadAssessmentState) {
  switch (state) {
    case "success":
      return "success";
    case "error":
      return "danger";
    case "running":
      return "warning";
    case "attention":
      return "warning";
    case "skipped":
      return "muted";
    case "idle":
    case "muted":
      return "muted";
  }
}

function defaultSidebarAssessmentSteps(): PackageLoadAssessmentStep[] {
  return packageLoadAssessmentCommands.map((command) => {
    return {
      caption: command.enabled
        ? command.command ?? ""
        : command.mutedCaption ?? "Not enabled",
      command: command.command,
      detail: null,
      enabled: command.enabled,
      finishedAt: null,
      id: command.id,
      label: command.label,
      output: null,
      startedAt: null,
      state: command.enabled ? "idle" : "muted",
      value: command.enabled ? "Pending" : "Skipped",
    };
  });
}
