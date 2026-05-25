import React from "react";

import { AppShell } from "@/app/app-shell";
import {
  listenCloseProject,
  listenOpenSettings,
  type PackageTree,
} from "@peregrine/desktop-runtime";

function App() {
  const [screen, setScreen] = React.useState<"workspace" | "settings">("workspace");
  const [packageTree, setPackageTree] = React.useState<PackageTree | null>(null);

  React.useEffect(() => {
    let unlistenSettings: (() => void) | undefined;
    let unlistenCloseProject: (() => void) | undefined;

    listenOpenSettings(() => setScreen("settings")).then((cleanup) => {
      unlistenSettings = cleanup;
    });
    listenCloseProject(() => {
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

export default App;
