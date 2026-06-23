import React from "react";
import { History, MessageSquare, Loader2 } from "lucide-react";
import {
  listAgentServerThreads,
} from "@peregrine/desktop-runtime";
import type { Thread } from "@peregrine/app-server-protocol/v2";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Button } from "@/components/ui/button";

export function SessionsSidebar({
  onSelectThread,
}: {
  onSelectThread: (threadId: string) => void;
}) {
  const [isLoading, setIsLoading] = React.useState(false);
  const [threads, setThreads] = React.useState<Thread[]>([]);

  React.useEffect(() => {
    let active = true;

    async function loadThreads() {
      setIsLoading(true);
      try {
        const listResponse = await listAgentServerThreads();
        const loadedThreads = listResponse.data;

        // Sort threads by updated_at descending if available, or just use order
        loadedThreads.sort((a, b) => {
          const aTime = a.updatedAt || 0;
          const bTime = b.updatedAt || 0;
          return bTime - aTime;
        });

        if (active) {
          setThreads(loadedThreads);
        }
      } catch (err) {
        console.error("Failed to list threads:", err);
      } finally {
        if (active) {
          setIsLoading(false);
        }
      }
    }

    void loadThreads();

    return () => {
      active = false;
    };
  }, []);

  return (
    <div className="flex h-full flex-col bg-[var(--app-chrome)]">
      <ScrollArea className="flex-1">
        <div className="flex flex-col p-2 space-y-1">
          {isLoading && threads.length === 0 ? (
            <div className="flex justify-center p-4">
              <Loader2 className="size-4 animate-spin text-muted-foreground" />
            </div>
          ) : threads.length === 0 ? (
            <div className="p-4 text-center text-sm text-muted-foreground">
              No previous sessions
            </div>
          ) : (
            threads.map((thread) => (
              <Button
                key={thread.id}
                variant="ghost"
                className="w-full justify-start px-2 py-1.5 h-auto whitespace-normal text-left"
                onClick={() => onSelectThread(thread.id)}
              >
                <div className="flex flex-col items-start w-full gap-1 overflow-hidden">
                  <div className="flex items-center gap-2 w-full">
                    <MessageSquare className="size-3.5 shrink-0 text-muted-foreground" />
                    <span className="text-xs font-medium truncate">
                      {thread.agentNickname || thread.id.slice(0, 8)}
                    </span>
                  </div>
                  {thread.preview && (
                    <span className="text-[10px] text-muted-foreground line-clamp-2 w-full pr-1">
                      {thread.preview}
                    </span>
                  )}
                </div>
              </Button>
            ))
          )}
        </div>
      </ScrollArea>
    </div>
  );
}
