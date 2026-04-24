import React from "react";

import { EmptyProjectScreen } from "@/features/empty-project/empty-project-screen";
import type { PackageTree } from "@/features/empty-project/filesystem-tree";
import { ProjectWorkspace } from "@/features/project-workspace/project-workspace";

export function Workspace() {
  const [packageTree, setPackageTree] = React.useState<PackageTree | null>(null);

  if (packageTree) {
    return <ProjectWorkspace packageTree={packageTree} />;
  }

  return <EmptyProjectScreen onProjectSelected={setPackageTree} />;
}
