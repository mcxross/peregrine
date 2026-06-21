import { useEffect, useRef } from "react";
import { SlashCommandDef, SLASH_COMMANDS } from "./slash-commands";
import { ScrollArea } from "../../components/ui/scroll-area";
import { cn } from "../../lib/utils";
interface SlashCommandPopupProps {
  input: string;
  onSelect: (command: SlashCommandDef) => void;
  selectedIndex: number;
}
export function SlashCommandPopup({
  input,
  onSelect,
  selectedIndex,
}: SlashCommandPopupProps) {
  const selectedRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (selectedRef.current) {
      selectedRef.current.scrollIntoView({ block: "nearest" });
    }
  }, [selectedIndex]);
  // Only show if the input starts with "/"
  if (!input.startsWith("/")) {
    return null;
  }
  const query = input.slice(1).toLowerCase();
  const filteredCommands = SLASH_COMMANDS.filter((c) =>
    c.command.toLowerCase().includes(query),
  );
  if (filteredCommands.length === 0) {
    return null;
  }
  return (
    <div className="absolute left-0 top-full z-50 mt-2 w-full rounded-xl border border-[color:var(--app-border)] bg-[var(--app-panel)] p-1 shadow-lg backdrop-blur-xl">
      <ScrollArea className="h-64">
        <div className="flex flex-col gap-0.5 p-1">
          {filteredCommands.map((command, idx) => (
            <button
              key={command.command}
              ref={idx === selectedIndex ? selectedRef : null}
              onMouseDown={() => onSelect(command)}
              onMouseDown={(e) => {
                e.preventDefault();
                onSelect(command);
              }}
              className={cn(
                "flex flex-col items-start rounded-lg px-3 py-2 text-left text-sm transition-colors",
                idx === selectedIndex
                  ? "bg-accent text-accent-foreground"
                  : "hover:bg-muted",
              )}
            >
              <span className="font-semibold">/{command.command}</span>
              <span className="text-xs text-muted-foreground">
                {command.description}
              </span>
            </button>
          ))}
        </div>
      </ScrollArea>
    </div>
  );
}
