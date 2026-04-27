import { cn } from "@/lib/utils";
import type { LayoutSettings } from "@/layout/layout-store";

type SidebarProps = {
  layout: LayoutSettings;
};

export function Sidebar({ layout }: SidebarProps) {
  if (!layout.sidebarVisible) {
    return null;
  }

  return (
    <aside
      className={cn(
        "flex min-h-0 flex-col border-r border-[color:var(--app-border)] bg-[var(--app-panel)] text-foreground",
        layout.density === "compact" ? "w-[240px]" : "w-[280px]",
      )}
    >
      <div className="min-h-0 flex-1" />
    </aside>
  );
}
