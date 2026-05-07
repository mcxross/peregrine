import React from "react";
import { listen } from "@tauri-apps/api/event";

import { AppShell } from "@/app/app-shell";
import type { PackageTree } from "@/features/empty-project/filesystem-tree";

function App() {
  const [screen, setScreen] = React.useState<"workspace" | "settings">("workspace");
  const [packageTree, setPackageTree] = React.useState<PackageTree | null>(null);

  React.useEffect(() => {
    let unlistenSettings: (() => void) | undefined;
    let unlistenCloseProject: (() => void) | undefined;

    listen(OPEN_SETTINGS_EVENT, () => setScreen("settings")).then((cleanup) => {
      unlistenSettings = cleanup;
    });
    listen(CLOSE_PROJECT_EVENT, () => {
      setPackageTree(null);
      setScreen("workspace");
    }).then((cleanup) => {
      unlistenCloseProject = cleanup;
    });

    return () => {
      unlistenSettings?.();
      unlistenCloseProject?.();
    };
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
const CLOSE_PROJECT_EVENT = "close-project";

export default App;
