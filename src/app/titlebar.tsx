import { cn } from "@/lib/utils";
import type { LayoutSettings } from "@/layout/layout-store";
import { trafficLightInset } from "@/layout/window-chrome";

type TitlebarProps = {
  layout: LayoutSettings;
  sectionTitle: string;
  detail?: string;
};

export function Titlebar({ layout, sectionTitle, detail }: TitlebarProps) {
  return (
    <header
      data-tauri-drag-region
      className={cn(
        "grid h-[52px] grid-cols-[280px_minmax(0,1fr)_auto] border-b bg-background/95 text-foreground backdrop-blur",
        layout.chrome === "compact" && "h-11 grid-cols-[220px_minmax(0,1fr)_auto]",
      )}
    >
      <div
        data-tauri-drag-region
        className="flex min-w-0 items-center gap-2 border-r px-3"
        style={{ paddingLeft: trafficLightInset }}
      >
        <div data-tauri-drag-region className="min-w-0">
          <div className="truncate text-sm font-semibold leading-none">Peregrine</div>
          <div className="mt-1 truncate text-xs text-muted-foreground">Move Security</div>
        </div>
      </div>

      <div data-tauri-drag-region className="flex min-w-0 items-center gap-2 px-4">
        <div data-tauri-drag-region className="min-w-0">
          <div className="truncate text-sm font-medium leading-none">{sectionTitle}</div>
          {detail ? (
            <div className="mt-1 truncate text-xs text-muted-foreground">{detail}</div>
          ) : null}
        </div>
      </div>

      <div className="flex items-center gap-2 px-3" />
    </header>
  );
}
