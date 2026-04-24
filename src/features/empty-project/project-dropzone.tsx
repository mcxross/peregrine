import { Download, FolderOpen } from "lucide-react";

import { Button } from "@/components/ui/button";

type ProjectDropzoneProps = {
  onOpenProject?: () => void;
};

export function ProjectDropzone({ onOpenProject }: ProjectDropzoneProps) {
  return (
    <section className="rounded-lg border bg-card p-5 text-card-foreground shadow-sm">
      <div className="flex min-h-[220px] flex-col items-center justify-center rounded-md border border-dashed border-muted-foreground/35 px-6 py-8 text-center">
        <div className="mb-4 flex size-16 items-center justify-center rounded-full border bg-muted text-muted-foreground">
          <FolderOpen className="size-8" aria-hidden="true" />
        </div>

        <h2 className="text-xl font-semibold tracking-tight">
          Drop your Move package here
        </h2>
        <p className="mt-2 max-w-md text-sm leading-6 text-muted-foreground">
          Move.toml, sources/, tests/, and package dependencies will be scanned
          locally.
        </p>

        <Button className="mt-5" onClick={onOpenProject}>
          <FolderOpen aria-hidden="true" />
          Open Move Package
        </Button>
      </div>

      <div className="mt-4 flex items-center justify-center gap-2 text-sm text-muted-foreground">
        <Download className="size-4" aria-hidden="true" />
        <span>You can also drag and drop a folder anywhere in this area</span>
      </div>
    </section>
  );
}
