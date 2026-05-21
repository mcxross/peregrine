import React from "react";
import { ChevronDown, Copy, RefreshCw } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import {
  networkOptions,
  suiGraphQlUrlForSelection,
  suiNetworkLabel,
  type NetworkId,
  type SuiNetworkSelection,
} from "@/app/sui-network";
import {
  loadSuiWalletSummary,
  type SuiWalletSummary,
} from "@/features/empty-project/filesystem-tree";
import { cn } from "@/lib/utils";

type SuiNetworkSelectorProps = {
  align?: "start" | "end";
  buttonId?: string;
  className?: string;
  contentClassName?: string;
  network: SuiNetworkSelection;
  onNetworkChange: (network: SuiNetworkSelection) => void;
  size?: "compact" | "default";
};

export function SuiNetworkSelector({
  align = "end",
  buttonId,
  className,
  contentClassName,
  network,
  onNetworkChange,
  size = "compact",
}: SuiNetworkSelectorProps) {
  const [customGraphQlDraft, setCustomGraphQlDraft] = React.useState(network.customGraphQlUrl ?? "");
  const [walletSummary, setWalletSummary] = React.useState<SuiWalletSummary | null>(null);
  const [walletError, setWalletError] = React.useState<string | null>(null);
  const [isLoadingWallet, setIsLoadingWallet] = React.useState(false);
  const label = suiNetworkLabel(network);
  const graphQlUrl = suiGraphQlUrlForSelection(network);
  const activeAddress = walletSummary?.activeAddress ?? null;
  const balanceLabel = walletSummary?.balance
    ? formatSuiBalance(walletSummary.balance.totalBalanceMist)
    : null;

  const refreshWalletSummary = React.useCallback(async () => {
    setIsLoadingWallet(true);
    setWalletError(null);

    try {
      setWalletSummary(await loadSuiWalletSummary(graphQlUrl));
    } catch (error) {
      setWalletSummary(null);
      setWalletError(getErrorMessage(error));
    } finally {
      setIsLoadingWallet(false);
    }
  }, [graphQlUrl]);

  React.useEffect(() => {
    setCustomGraphQlDraft(network.customGraphQlUrl ?? "");
  }, [network.customGraphQlUrl]);

  React.useEffect(() => {
    void refreshWalletSummary();
  }, [refreshWalletSummary]);

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          aria-label={`Switch network. Current network: ${label}`}
          className={cn(
            "gap-2 rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] font-medium text-foreground hover:bg-[var(--app-elevated)]",
            size === "compact" ? "h-8 px-2.5 text-xs" : "h-10 justify-between px-3 text-sm",
            className,
          )}
          id={buttonId}
          type="button"
          variant="ghost"
        >
          <span className="flex min-w-0 items-center gap-2">
            <span className="relative flex size-2.5 shrink-0">
              <span className="absolute inline-flex size-full rounded-full bg-emerald-400 opacity-40" />
              <span className="relative inline-flex size-2.5 rounded-full bg-emerald-400" />
            </span>
            <span className="truncate">{label}</span>
          </span>
          <ChevronDown className={cn("shrink-0 text-muted-foreground", size === "compact" ? "size-3.5" : "size-4")} aria-hidden="true" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align={align} className={cn("w-96 max-w-[calc(100vw-2rem)]", contentClassName)}>
        <div className="grid gap-3 p-2">
          <div className="flex items-start justify-between gap-3">
            <div className="min-w-0">
              <DropdownMenuLabel className="px-0 py-0 text-xs text-muted-foreground">
                Active Sui account
              </DropdownMenuLabel>
              <div className="mt-1 truncate text-sm font-medium text-foreground">
                {walletSummary?.activeAlias ?? (activeAddress ? "Active address" : "No active address")}
              </div>
            </div>
            <Button
              disabled={isLoadingWallet}
              onClick={() => void refreshWalletSummary()}
              size="icon-xs"
              title="Refresh wallet summary"
              type="button"
              variant="outline"
            >
              <RefreshCw className={cn("size-3", isLoadingWallet && "animate-spin")} aria-hidden="true" />
            </Button>
          </div>

          {activeAddress ? (
            <div className="grid gap-2 rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] p-2">
              <div className="flex min-w-0 items-center justify-between gap-2">
                <span className="truncate font-mono text-xs text-muted-foreground" title={activeAddress}>
                  {truncateMiddle(activeAddress, 16, 12)}
                </span>
                <Button
                  onClick={() => void copyToClipboard(activeAddress)}
                  size="icon-xs"
                  title="Copy active address"
                  type="button"
                  variant="ghost"
                >
                  <Copy className="size-3" aria-hidden="true" />
                </Button>
              </div>
              <div className="flex items-center justify-between gap-3 text-xs">
                <span className="text-muted-foreground">Available SUI</span>
                <span className="font-mono text-foreground">
                  {isLoadingWallet ? "Loading..." : balanceLabel ?? "Unavailable"}
                </span>
              </div>
              {!isLoadingWallet && walletSummary?.balanceError ? (
                <p className="text-[11px] text-muted-foreground">
                  {walletSummary.balanceError}
                </p>
              ) : null}
            </div>
          ) : (
            <p className="rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-3 py-2 text-xs text-muted-foreground">
              Generate or import a Sui key in Settings to show an active address here.
            </p>
          )}

          {walletError ? (
            <p className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
              {walletError}
            </p>
          ) : null}
        </div>

        <DropdownMenuSeparator />

        <DropdownMenuLabel className="text-xs text-muted-foreground">
          Sui network
        </DropdownMenuLabel>
        <DropdownMenuRadioGroup
          value={network.id}
          onValueChange={(value) => {
            onNetworkChange({
              id: value as NetworkId,
              customGraphQlUrl: network.customGraphQlUrl,
            });
          }}
        >
          {networkOptions.map((option) => (
            <DropdownMenuRadioItem key={option.id} value={option.id}>
              <span className="flex min-w-0 flex-1 items-center justify-between gap-3">
                <span>{option.label}</span>
                {option.id === "testnet" ? (
                  <span className="text-[11px] text-muted-foreground">default</span>
                ) : null}
              </span>
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>

        <DropdownMenuSeparator />

        <div className="grid gap-2 p-2" onKeyDown={(event) => event.stopPropagation()}>
          <DropdownMenuLabel className="px-0 py-0 text-xs text-muted-foreground">
            Custom GraphQL endpoint
          </DropdownMenuLabel>
          <Input
            className="h-8 text-xs"
            onChange={(event) => setCustomGraphQlDraft(event.target.value)}
            onKeyDown={(event) => event.stopPropagation()}
            placeholder="https://graphql.testnet.sui.io/graphql"
            value={customGraphQlDraft}
          />
          <Button
            className="h-8 justify-center text-xs"
            disabled={!customGraphQlDraft.trim()}
            onClick={() => {
              onNetworkChange({
                id: "custom",
                customGraphQlUrl: customGraphQlDraft.trim(),
              });
            }}
            type="button"
            variant="outline"
          >
            Use custom GraphQL
          </Button>
          {network.id === "custom" && network.customGraphQlUrl ? (
            <p className="truncate text-[11px] text-muted-foreground" title={network.customGraphQlUrl}>
              Active: {network.customGraphQlUrl}
            </p>
          ) : null}
        </div>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function formatSuiBalance(mist: string) {
  try {
    const value = BigInt(mist);
    const whole = value / 1_000_000_000n;
    const fraction = value % 1_000_000_000n;

    if (fraction === 0n) {
      return `${whole.toLocaleString()} SUI`;
    }

    const fractionText = fraction.toString().padStart(9, "0").slice(0, 4).replace(/0+$/, "");
    return `${whole.toLocaleString()}.${fractionText || "0"} SUI`;
  } catch {
    return `${mist} MIST`;
  }
}

function truncateMiddle(value: string, prefixLength: number, suffixLength: number) {
  if (value.length <= prefixLength + suffixLength + 1) {
    return value;
  }

  return `${value.slice(0, prefixLength)}...${value.slice(-suffixLength)}`;
}

async function copyToClipboard(value: string) {
  await navigator.clipboard.writeText(value);
}

function getErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  return typeof error === "string" ? error : "Could not load Sui wallet summary.";
}
