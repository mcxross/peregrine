import React from "react";
import { listen } from "@tauri-apps/api/event";

import { AppShell } from "@/app/app-shell";

function App() {
  const [screen, setScreen] = React.useState<"workspace" | "settings">("workspace");

  React.useEffect(() => {
    let unlisten: (() => void) | undefined;

    listen(OPEN_SETTINGS_EVENT, () => setScreen("settings")).then((cleanup) => {
      unlisten = cleanup;
    });

    return () => unlisten?.();
  }, []);

  return (
    <AppShell screen={screen} onCloseSettings={() => setScreen("workspace")} />
  );
}

const OPEN_SETTINGS_EVENT = "open-settings";

export default App;
