import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { cn } from "@/lib/utils";
import { useTheme } from "@/theme/theme-provider";
import type { ThemeId, ThemeMode } from "@/theme/themes";

type SettingsScreenProps = {
  onBack: () => void;
};

const modeOptions: { value: ThemeMode; label: string }[] = [
  { value: "system", label: "System" },
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
];

export function SettingsScreen({ onBack }: SettingsScreenProps) {
  const { themes, themeId, mode, resolvedMode, setMode, setThemeId } = useTheme();

  return (
    <main className="h-full min-h-0 overflow-auto bg-background text-foreground">
      <div className="mx-auto flex w-full max-w-5xl flex-col gap-6 px-5 py-5 sm:px-8 sm:py-7">
        <header className="flex items-center justify-between gap-4 border-b pb-5">
          <div>
            <h1 className="text-xl font-semibold tracking-tight">Settings</h1>
            <p className="text-sm text-muted-foreground">
              Appearance · {resolvedMode}
            </p>
          </div>
          <Button variant="outline" onClick={onBack}>
            Done
          </Button>
        </header>

        <section className="grid gap-4 md:grid-cols-[240px_1fr]">
          <Card className="h-fit">
            <CardHeader>
              <CardTitle>Appearance</CardTitle>
              <CardDescription>Mode</CardDescription>
            </CardHeader>
            <CardContent>
              <div className="grid grid-cols-3 gap-1 rounded-lg border bg-muted p-1 md:grid-cols-1">
                {modeOptions.map((option) => (
                  <Button
                    key={option.value}
                    variant={mode === option.value ? "default" : "ghost"}
                    size="sm"
                    onClick={() => setMode(option.value)}
                  >
                    {option.label}
                  </Button>
                ))}
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Themes</CardTitle>
              <CardDescription>shadcn families</CardDescription>
            </CardHeader>
            <CardContent>
              <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
                {themes.map((theme) => (
                  <button
                    key={theme.id}
                    type="button"
                    onClick={() => setThemeId(theme.id as ThemeId)}
                    className={cn(
                      "group rounded-lg border bg-card p-3 text-left text-card-foreground transition hover:border-ring hover:shadow-sm focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50",
                      themeId === theme.id && "border-ring ring-[3px] ring-ring/20",
                    )}
                  >
                    <div className="mb-3 flex items-center justify-between gap-2">
                      <div>
                        <div className="font-medium leading-none">{theme.name}</div>
                        <div className="mt-1 text-xs capitalize text-muted-foreground">
                          {theme.family}
                        </div>
                      </div>
                      <span
                        className="size-5 rounded-full border shadow-xs"
                        style={{ background: theme.swatch }}
                      />
                    </div>

                    <div className="overflow-hidden rounded-md border">
                      <div className="flex h-9 items-center gap-1.5 px-2">
                        <span className="h-2.5 w-10 rounded-full" style={{ background: theme.light.primary }} />
                        <span className="h-2.5 w-6 rounded-full" style={{ background: theme.light.accent }} />
                        <span className="h-2.5 w-4 rounded-full" style={{ background: theme.light.border }} />
                      </div>
                      <div className="flex h-9 items-center gap-1.5 border-t px-2">
                        <span className="h-2.5 w-10 rounded-full" style={{ background: theme.dark.primary }} />
                        <span className="h-2.5 w-6 rounded-full" style={{ background: theme.dark.accent }} />
                        <span className="h-2.5 w-4 rounded-full" style={{ background: theme.dark.border }} />
                      </div>
                    </div>
                  </button>
                ))}
              </div>
            </CardContent>
          </Card>
        </section>
      </div>
    </main>
  );
}
