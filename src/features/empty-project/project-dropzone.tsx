import { Download, FolderOpen, PackagePlus, Plus } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";

type ProjectDropzoneProps = {
  isLoading?: boolean;
  onCreateProject?: () => void;
  onImportPackage?: () => void;
  onOpenProject?: () => void;
  networkLabel?: string;
};

export function ProjectDropzone({
  isLoading = false,
  onCreateProject,
  onImportPackage,
  onOpenProject,
  networkLabel,
}: ProjectDropzoneProps) {
  return (
    <Card className="mx-auto w-full max-w-[640px] rounded-md p-4 shadow-none">
      <div className="flex min-h-[248px] flex-col items-center justify-center rounded-md border border-dashed border-muted-foreground/30 bg-[var(--app-surface)] px-8 py-7 text-center shadow-[inset_0_1px_0_rgba(255,255,255,0.035)] sm:min-h-[328px]">
        <div className="flex size-14 shrink-0 items-center justify-center rounded-md border border-[color:var(--app-border)] bg-[var(--app-elevated)] text-muted-foreground shadow-[inset_0_1px_0_rgba(255,255,255,0.05)]">
          <FolderOpen className="size-7" aria-hidden="true" />
        </div>

        <h2 className="mt-5 text-lg font-semibold tracking-tight">
          Open a Move package
        </h2>
        <p className="mt-2 max-w-[420px] text-sm leading-6 text-muted-foreground">
          Drop a folder here or choose one from the file picker. Move.toml,
          sources/, tests/, and dependencies are scanned locally.
        </p>

        <div className="mt-5 flex flex-col items-center justify-center gap-2 sm:flex-row sm:flex-wrap">
          <Button onClick={onOpenProject} disabled={isLoading}>
            <FolderOpen aria-hidden="true" />
            {isLoading ? "Opening..." : "Open Move Package"}
          </Button>
          <Button variant="outline" onClick={onImportPackage} disabled={isLoading}>
            <PackagePlus aria-hidden="true" />
            Import Package ID
          </Button>
          <Button variant="outline" onClick={onCreateProject} disabled={isLoading}>
            <Plus aria-hidden="true" />
            New Move Package
          </Button>
        </div>

        <div className="mt-5 flex items-center justify-center gap-2 text-xs text-muted-foreground">
          <Download className="size-4" aria-hidden="true" />
          <span>
            {networkLabel ? `Imports use ${networkLabel} GraphQL.` : "Drag and drop uses the same local scanner."}
          </span>
        </div>
      </div>
    </Card>
  );
}
