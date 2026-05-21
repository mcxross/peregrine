import { Info } from "lucide-react";

import { cn } from "@/lib/utils";

type MoveSourceUnavailableNoticeProps = {
  className?: string;
  message: string;
  title: string;
  variant?: "centered" | "inline";
};

export function MoveSourceUnavailableNotice({
  className,
  message,
  title,
  variant = "centered",
}: MoveSourceUnavailableNoticeProps) {
  const notice = (
    <div
      className={cn(
        "grid max-w-xl grid-cols-[auto_minmax(0,1fr)] gap-3 rounded-md border border-[color:var(--app-border)] bg-card p-4 text-left shadow-sm",
        variant === "inline" && "max-w-[640px] p-3",
      )}
    >
      <div className="mt-0.5 flex size-7 items-center justify-center rounded-md border border-[color:var(--app-border)] bg-[var(--app-subtle)] text-muted-foreground">
        <Info className="size-4" aria-hidden="true" />
      </div>
      <div className="min-w-0">
        <div className="text-sm font-semibold text-foreground">{title}</div>
        <p className="mt-1.5 text-sm leading-6 text-muted-foreground">{message}</p>
      </div>
    </div>
  );

  if (variant === "inline") {
    return <div className={cn("mt-3", className)}>{notice}</div>;
  }

  return (
    <div className={cn("grid h-full min-h-0 place-items-center px-6", className)}>
      {notice}
    </div>
  );
}
