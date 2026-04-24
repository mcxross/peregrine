import { Titlebar } from "@/app/titlebar";
import { Sidebar } from "@/app/sidebar";
import { Workspace } from "@/app/workspace";
import { defaultLayoutSettings } from "@/layout/layout-store";
import { SettingsScreen } from "@/screens/settings-screen";

type AppShellProps = {
  screen: "workspace" | "settings";
  onCloseSettings: () => void;
};

export function AppShell({ screen, onCloseSettings }: AppShellProps) {
  const layout = defaultLayoutSettings;
  const isSettings = screen === "settings";
  const showSidebar = isSettings;

  return (
    <main className="grid h-svh grid-rows-[52px_1fr] overflow-hidden bg-background text-foreground">
      <Titlebar
        layout={layout}
        sectionTitle={isSettings ? "Settings" : "Peregrine"}
        detail={isSettings ? "Appearance" : undefined}
      />

      <section className="flex min-h-0">
        {showSidebar ? <Sidebar layout={layout} /> : null}
        <div className="min-w-0 flex-1">
          {isSettings ? <SettingsScreen onBack={onCloseSettings} /> : <Workspace />}
        </div>
      </section>
    </main>
  );
}
