import { FileCode2, ShieldCheck } from "lucide-react";

import type {
  MoveModule,
  MovePackage,
} from "@/features/empty-project/filesystem-tree";
import { cn } from "@/lib/utils";

export type SelectedMoveModule = {
  moveModule: MoveModule;
  movePackage: MovePackage;
};

type ModuleSignatureScreenProps = {
  selectedModule: SelectedMoveModule;
};

export function ModuleSignatureScreen({
  selectedModule,
}: ModuleSignatureScreenProps) {
  const { moveModule, movePackage } = selectedModule;

  return (
    <section className="grid h-full min-h-0 grid-rows-[auto_minmax(0,1fr)] bg-background">
      <header className="border-b px-6 py-4">
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <ShieldCheck className="size-4" aria-hidden="true" />
          Module Surface
        </div>
        <h2 className="mt-2 truncate text-2xl font-semibold">{moveModule.name}</h2>
        <p className="mt-1 truncate text-sm text-muted-foreground">
          {movePackage.name} / {moveModule.filePath}
        </p>
      </header>

      <div className="min-h-0 overflow-auto px-6 py-5">
        {moveModule.functions.length ? (
          <div className="space-y-3">
            {moveModule.functions.map((signature) => (
              <article
                key={`${signature.name}-${signature.signature}`}
                className="rounded-lg border bg-card p-4"
              >
                <div className="flex items-center justify-between gap-3">
                  <div className="flex min-w-0 items-center gap-2">
                    <FileCode2 className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
                    <h3 className="truncate text-sm font-semibold">{signature.name}</h3>
                  </div>
                  <div className="flex shrink-0 items-center gap-2">
                    <Badge tone={visibilityTone(signature.visibility)}>
                      {signature.visibility}
                    </Badge>
                    {signature.isEntry ? <Badge tone="entry">entry</Badge> : null}
                  </div>
                </div>
                <pre className="mt-3 overflow-auto rounded-md bg-muted/40 p-3 text-xs leading-5 text-foreground">
                  <code>{signature.signature}</code>
                </pre>
              </article>
            ))}
          </div>
        ) : (
          <div className="flex h-full min-h-48 items-center justify-center rounded-lg border bg-muted/20 text-sm text-muted-foreground">
            No function signatures found for this module.
          </div>
        )}
      </div>
    </section>
  );
}

function Badge({
  children,
  tone,
}: {
  children: string;
  tone: "entry" | "private" | "public";
}) {
  return (
    <span
      className={cn(
        "rounded px-2 py-0.5 text-xs font-medium",
        tone === "public" && "bg-emerald-500/10 text-emerald-300",
        tone === "private" && "bg-muted text-muted-foreground",
        tone === "entry" && "bg-primary/15 text-primary",
      )}
    >
      {children}
    </span>
  );
}

function visibilityTone(visibility: string) {
  return visibility === "private" ? "private" : "public";
}
