import React from "react";
import { ChevronDown, Copy, Plus, RefreshCw, Trash2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
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
  suiGraphQlUrlForEnv,
  suiGraphQlUrlForSelection,
  suiNetworkLabel,
  suiNetworkSelectionFromEnv,
  type SuiNetworkSelection,
} from "@/app/sui-network";
import {
  addSuiNetworkEnv,
  loadSuiNetworkState,
  loadSuiWalletSummary,
  removeSuiNetworkEnv,
  setActiveSuiNetworkEnv,
  type SuiNetworkEnv,
  type SuiNetworkState,
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
  const [networkState, setNetworkState] = React.useState<SuiNetworkState | null>(null);
  const [networkError, setNetworkError] = React.useState<string | null>(null);
  const [walletSummary, setWalletSummary] = React.useState<SuiWalletSummary | null>(null);
  const [walletError, setWalletError] = React.useState<string | null>(null);
  const [isLoadingNetwork, setIsLoadingNetwork] = React.useState(false);
  const [isLoadingWallet, setIsLoadingWallet] = React.useState(false);
  const [isMutatingNetwork, setIsMutatingNetwork] = React.useState(false);
  const [isAddEnvOpen, setIsAddEnvOpen] = React.useState(false);
  const [removeEnv, setRemoveEnv] = React.useState<SuiNetworkEnv | null>(null);
  const envs = networkState?.envs ?? [];
  const selectedEnv = React.useMemo(
    () => envs.find((env) => env.isActive)
      ?? envs.find((env) => env.alias === network.id)
      ?? envs[0]
      ?? null,
    [envs, network.id],
  );
  const visibleEnvs = React.useMemo(() => {
    const firstEnvs = envs.slice(0, 3);

    if (!selectedEnv || firstEnvs.some((env) => env.alias === selectedEnv.alias)) {
      return firstEnvs;
    }

    return [
      selectedEnv,
      ...envs.filter((env) => env.alias !== selectedEnv.alias).slice(0, 2),
    ];
  }, [envs, selectedEnv]);
  const hiddenEnvCount = Math.max(envs.length - visibleEnvs.length, 0);
  const label = selectedEnv ? suiNetworkLabel(suiNetworkSelectionFromEnv(selectedEnv)) : suiNetworkLabel(network);
  const graphQlUrl = selectedEnv ? suiGraphQlUrlForEnv(selectedEnv) : suiGraphQlUrlForSelection(network);
  const activeAddress = walletSummary?.activeAddress ?? null;
  const balanceLabel = walletSummary?.balance
    ? formatSuiBalance(walletSummary.balance.totalBalanceMist)
    : null;
  const disabled = isLoadingNetwork || isMutatingNetwork;

  const refreshNetworkState = React.useCallback(async () => {
    setIsLoadingNetwork(true);
    setNetworkError(null);

    try {
      const state = await loadSuiNetworkState();
      setNetworkState(state);
      const activeEnv = state.envs.find((env) => env.isActive) ?? state.envs[0] ?? null;

      if (activeEnv) {
        onNetworkChange(suiNetworkSelectionFromEnv(activeEnv));
      }
    } catch (error) {
      setNetworkError(getErrorMessage(error, "Could not load Sui network environments."));
    } finally {
      setIsLoadingNetwork(false);
    }
  }, [onNetworkChange]);

  const refreshWalletSummary = React.useCallback(async () => {
    setIsLoadingWallet(true);
    setWalletError(null);

    try {
      setWalletSummary(await loadSuiWalletSummary(graphQlUrl));
    } catch (error) {
      setWalletSummary(null);
      setWalletError(getErrorMessage(error, "Could not load Sui wallet summary."));
    } finally {
      setIsLoadingWallet(false);
    }
  }, [graphQlUrl]);

  React.useEffect(() => {
    void refreshNetworkState();
  }, [refreshNetworkState]);

  React.useEffect(() => {
    void refreshWalletSummary();
  }, [refreshWalletSummary]);

  const selectEnv = React.useCallback(
    async (alias: string) => {
      if (disabled || alias === selectedEnv?.alias) {
        return;
      }

      setIsMutatingNetwork(true);
      setNetworkError(null);

      try {
        const state = await setActiveSuiNetworkEnv(alias);
        setNetworkState(state);
        const activeEnv = state.envs.find((env) => env.isActive) ?? null;

        if (activeEnv) {
          onNetworkChange(suiNetworkSelectionFromEnv(activeEnv));
        }
      } catch (error) {
        setNetworkError(getErrorMessage(error, "Could not switch Sui network environment."));
      } finally {
        setIsMutatingNetwork(false);
      }
    },
    [disabled, onNetworkChange, selectedEnv?.alias],
  );

  const addEnv = React.useCallback(
    async (request: { alias: string; rpc: string; ws: string }) => {
      setIsMutatingNetwork(true);
      setNetworkError(null);

      try {
        const addedState = await addSuiNetworkEnv({
          alias: request.alias,
          basicAuth: null,
          rpc: request.rpc,
          ws: request.ws.trim() || null,
        });
        setNetworkState(addedState);
        const state = await setActiveSuiNetworkEnv(request.alias.trim());
        setNetworkState(state);
        const activeEnv = state.envs.find((env) => env.isActive) ?? null;

        if (activeEnv) {
          onNetworkChange(suiNetworkSelectionFromEnv(activeEnv));
        }
      } catch (error) {
        setNetworkError(getErrorMessage(error, "Could not add Sui network environment."));
        throw error;
      } finally {
        setIsMutatingNetwork(false);
      }
    },
    [onNetworkChange],
  );

  const removeSelectedEnv = React.useCallback(
    async (alias: string, confirmation: string) => {
      setIsMutatingNetwork(true);
      setNetworkError(null);

      try {
        const state = await removeSuiNetworkEnv(alias, confirmation);
        setNetworkState(state);
        const activeEnv = state.envs.find((env) => env.isActive) ?? state.envs[0] ?? null;

        if (activeEnv) {
          onNetworkChange(suiNetworkSelectionFromEnv(activeEnv));
        }
      } catch (error) {
        setNetworkError(getErrorMessage(error, "Could not remove Sui network environment."));
        throw error;
      } finally {
        setIsMutatingNetwork(false);
      }
    },
    [onNetworkChange],
  );

  return (
    <>
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
                disabled={isLoadingWallet || isLoadingNetwork}
                onClick={() => {
                  void refreshNetworkState();
                  void refreshWalletSummary();
                }}
                size="icon-xs"
                title="Refresh Sui account and environments"
                type="button"
                variant="outline"
              >
                <RefreshCw className={cn("size-3", (isLoadingWallet || isLoadingNetwork) && "animate-spin")} aria-hidden="true" />
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

            {walletError || networkError ? (
              <p className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
                {walletError ?? networkError}
              </p>
            ) : null}
          </div>

          <DropdownMenuSeparator />

          <div className="flex items-center justify-between px-2 py-1.5">
            <DropdownMenuLabel className="px-0 py-0 text-xs text-muted-foreground">
              Sui environment
            </DropdownMenuLabel>
            <Button
              disabled={disabled}
              onClick={() => setIsAddEnvOpen(true)}
              size="xs"
              type="button"
              variant="outline"
            >
              <Plus aria-hidden="true" />
              Add
            </Button>
          </div>

          {envs.length ? (
            <DropdownMenuRadioGroup
              value={selectedEnv?.alias ?? ""}
              onValueChange={(alias) => void selectEnv(alias)}
            >
              {visibleEnvs.map((env) => (
                <DropdownMenuRadioItem className="items-start gap-2 py-2 pr-2" key={env.alias} value={env.alias}>
                  <span className="grid min-w-0 flex-1 gap-1">
                    <span className="flex min-w-0 items-center justify-between gap-3">
                      <span className="truncate font-medium">{env.alias}</span>
                      {env.isBuiltin ? (
                        <span className="text-[11px] text-muted-foreground">default</span>
                      ) : null}
                    </span>
                    <span className="truncate font-mono text-[11px] text-muted-foreground" title={env.rpc}>
                      {env.rpc}
                    </span>
                  </span>
                  {env.canRemove ? (
                    <Button
                      className="mt-0.5 shrink-0"
                      disabled={disabled}
                      onClick={(event) => {
                        event.preventDefault();
                        event.stopPropagation();
                        setRemoveEnv(env);
                      }}
                      onPointerDown={(event) => event.stopPropagation()}
                      size="icon-xs"
                      title={`Remove ${env.alias}`}
                      type="button"
                      variant="ghost"
                    >
                      <Trash2 className="size-3" aria-hidden="true" />
                    </Button>
                  ) : null}
                </DropdownMenuRadioItem>
              ))}
            </DropdownMenuRadioGroup>
          ) : (
            <p className="px-3 py-4 text-xs text-muted-foreground">
              No Sui environments are configured.
            </p>
          )}
          {hiddenEnvCount > 0 ? (
            <p className="px-3 pb-2 pt-1 text-[11px] text-muted-foreground">
              {hiddenEnvCount} more in Sui client config.
            </p>
          ) : null}
        </DropdownMenuContent>
      </DropdownMenu>

      <AddSuiEnvDialog
        disabled={isMutatingNetwork}
        onAdd={addEnv}
        onOpenChange={setIsAddEnvOpen}
        open={isAddEnvOpen}
      />
      <RemoveSuiEnvDialog
        disabled={isMutatingNetwork}
        env={removeEnv}
        onOpenChange={(open) => {
          if (!open) {
            setRemoveEnv(null);
          }
        }}
        onRemove={removeSelectedEnv}
      />
    </>
  );
}

function AddSuiEnvDialog({
  disabled,
  onAdd,
  onOpenChange,
  open,
}: {
  disabled: boolean;
  onAdd: (request: { alias: string; rpc: string; ws: string }) => Promise<void>;
  onOpenChange: (open: boolean) => void;
  open: boolean;
}) {
  const [alias, setAlias] = React.useState("");
  const [error, setError] = React.useState<string | null>(null);
  const [rpc, setRpc] = React.useState("");
  const [ws, setWs] = React.useState("");

  React.useEffect(() => {
    if (!open) {
      setAlias("");
      setError(null);
      setRpc("");
      setWs("");
    }
  }, [open]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Add Sui Environment</DialogTitle>
          <DialogDescription>
            Adds an environment to Sui’s standard client config.
          </DialogDescription>
        </DialogHeader>
        <div className="grid gap-4">
          <LabeledField label="Alias">
            <Input
              autoComplete="off"
              onChange={(event) => setAlias(event.target.value)}
              placeholder="my-env"
              value={alias}
            />
          </LabeledField>
          <LabeledField label="RPC URL">
            <Input
              autoComplete="off"
              onChange={(event) => setRpc(event.target.value)}
              placeholder="https://fullnode.testnet.sui.io:443"
              value={rpc}
            />
          </LabeledField>
          <LabeledField label="WebSocket URL">
            <Input
              autoComplete="off"
              onChange={(event) => setWs(event.target.value)}
              placeholder="Optional"
              value={ws}
            />
          </LabeledField>
          {error ? (
            <p className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
              {error}
            </p>
          ) : null}
        </div>
        <DialogFooter>
          <Button disabled={disabled} onClick={() => onOpenChange(false)} type="button" variant="outline">
            Cancel
          </Button>
          <Button
            disabled={disabled || !alias.trim() || !rpc.trim()}
            onClick={() => {
              setError(null);
              void onAdd({ alias, rpc, ws })
                .then(() => onOpenChange(false))
                .catch((error) => setError(getErrorMessage(error, "Could not add Sui environment.")));
            }}
            type="button"
          >
            Add
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function RemoveSuiEnvDialog({
  disabled,
  env,
  onOpenChange,
  onRemove,
}: {
  disabled: boolean;
  env: SuiNetworkEnv | null;
  onOpenChange: (open: boolean) => void;
  onRemove: (alias: string, confirmation: string) => Promise<void>;
}) {
  const [confirmation, setConfirmation] = React.useState("");
  const [error, setError] = React.useState<string | null>(null);

  React.useEffect(() => {
    if (!env) {
      setConfirmation("");
      setError(null);
    }
  }, [env]);

  return (
    <Dialog open={Boolean(env)} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Remove Sui Environment</DialogTitle>
          <DialogDescription>
            Removes this environment from Sui’s client config.
          </DialogDescription>
        </DialogHeader>
        {env ? (
          <div className="grid gap-4">
            <p className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
              Type `{env.alias}` to confirm removal.
            </p>
            <Input
              autoComplete="off"
              onChange={(event) => setConfirmation(event.target.value)}
              value={confirmation}
            />
            {error ? (
              <p className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
                {error}
              </p>
            ) : null}
          </div>
        ) : null}
        <DialogFooter>
          <Button disabled={disabled} onClick={() => onOpenChange(false)} type="button" variant="outline">
            Cancel
          </Button>
          <Button
            disabled={disabled || !env || confirmation.trim() !== env.alias}
            onClick={() => {
              if (!env) {
                return;
              }
              setError(null);
              void onRemove(env.alias, confirmation)
                .then(() => onOpenChange(false))
                .catch((error) => setError(getErrorMessage(error, "Could not remove Sui environment.")));
            }}
            type="button"
            variant="destructive"
          >
            Remove
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function LabeledField({
  children,
  label,
}: {
  children: React.ReactNode;
  label: string;
}) {
  return (
    <label className="grid gap-1.5 text-sm">
      <span className="font-medium text-foreground">{label}</span>
      {children}
    </label>
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

function getErrorMessage(error: unknown, fallback: string) {
  if (error instanceof Error) {
    return error.message;
  }

  return typeof error === "string" ? error : fallback;
}
