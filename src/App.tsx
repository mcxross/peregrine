import React from "react";
import { listen } from "@tauri-apps/api/event";

import { AppShell } from "@/app/app-shell";
import type { PackageTree } from "@/features/empty-project/filesystem-tree";

function App() {
  const [screen, setScreen] = React.useState<"workspace" | "settings">("workspace");
  const [packageTree, setPackageTree] = React.useState<PackageTree | null>(null);

  React.useEffect(() => {
    let unlisten: (() => void) | undefined;

    listen(OPEN_SETTINGS_EVENT, () => setScreen("settings")).then((cleanup) => {
      unlisten = cleanup;
    });

    return () => unlisten?.();
  }, []);

  return (
    <AppShell
      packageTree={packageTree}
      screen={screen}
      onCloseSettings={() => setScreen("workspace")}
      onProjectSelected={setPackageTree}
    />
  );
}

const OPEN_SETTINGS_EVENT = "open-settings";

export default App;
