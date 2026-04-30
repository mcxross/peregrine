import { Box, ChevronDown, FileCode2, X } from "lucide-react";
import React from "react";

import type {
  MoveModule,
  MovePackage,
} from "@/features/empty-project/filesystem-tree";
import { cn } from "@/lib/utils";

export type SelectedMoveModule = {
  moveModule: MoveModule;
  movePackage: MovePackage;
};

type ModuleSignatureScreenProps = {
  onClose?: () => void;
  selectedModule: SelectedMoveModule;
};

export function ModuleSignatureScreen({
  onClose,
  selectedModule,
}: ModuleSignatureScreenProps) {
  const { moveModule, movePackage } = selectedModule;
  const structs = moveModule.structs ?? [];
  const functions = moveModule.functions ?? [];
  const hasSurface = structs.length || functions.length;
  const [openFunctionKey, setOpenFunctionKey] = React.useState<string | null>(null);

  React.useEffect(() => {
    setOpenFunctionKey(null);
  }, [moveModule.filePath]);

  return (
    <section className="grid h-full min-h-0 grid-rows-[auto_minmax(0,1fr)] bg-[var(--app-window)]">
      <header className="flex min-w-0 items-center justify-between gap-4 border-b border-[color:var(--app-border)] px-6 py-2.5">
        <div className="min-w-0">
          <h2 className="truncate text-xl font-semibold leading-6">{moveModule.name}</h2>
          <p className="mt-0.5 truncate text-xs leading-5 text-muted-foreground">
            {movePackage.name} / {moveModule.filePath}
          </p>
        </div>
        {onClose ? (
          <button
            aria-label="Close module surface"
            className="inline-flex size-8 shrink-0 items-center justify-center rounded-md text-muted-foreground transition hover:bg-[var(--app-subtle)] hover:text-foreground"
            onClick={onClose}
            type="button"
          >
            <X className="size-4" aria-hidden="true" />
          </button>
        ) : null}
      </header>

      <div className="min-h-0 overflow-auto px-6 py-5">
        {hasSurface ? (
          <div className="space-y-6">
            <SurfaceSection
              count={structs.length}
              emptyText="No structs found for this module."
              title="Structs"
            >
              <div className="space-y-3">
                {structs.map((signature) => (
                  <article
                    key={`${signature.name}-${signature.signature}`}
                    className="rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] p-4"
                  >
                    <div className="flex items-center justify-between gap-3">
                      <div className="flex min-w-0 items-center gap-2">
                        <Box className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
                        <h3 className="truncate text-sm font-semibold">{signature.name}</h3>
                      </div>
                      <div className="flex shrink-0 flex-wrap justify-end gap-2">
                        {signature.abilities.length ? (
                          signature.abilities.map((ability) => (
                            <Badge key={ability} tone="ability">
                              {ability}
                            </Badge>
                          ))
                        ) : (
                          <Badge tone="private">no abilities</Badge>
                        )}
                      </div>
                    </div>
                    <pre className="mt-3 overflow-auto rounded-md bg-[var(--app-subtle)] p-3 text-xs leading-5 [font-family:'JetBrains_Mono','JetBrains_Mono_NL','JetBrains_Mono_NF',ui-monospace,SFMono-Regular,'SF_Mono',Menlo,Monaco,Consolas,'Liberation_Mono',monospace]">
                      <code>
                        <HighlightedMoveSignature source={signature.signature} />
                      </code>
                    </pre>
                  </article>
                ))}
              </div>
            </SurfaceSection>

            <SurfaceSection
              count={functions.length}
              emptyText="No function signatures found for this module."
              title="Functions"
            >
              <div className="space-y-3">
                {functions.map((signature) => (
                  <FunctionSignatureCard
                    key={`${signature.name}-${signature.signature}`}
                    isOpen={openFunctionKey === functionKey(signature)}
                    onToggle={() => {
                      const key = functionKey(signature);
                      setOpenFunctionKey((current) => current === key ? null : key);
                    }}
                    signature={signature}
                  />
                ))}
              </div>
            </SurfaceSection>
          </div>
        ) : (
          <div className="flex h-full min-h-48 items-center justify-center rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] text-sm text-muted-foreground">
            No structs or function signatures found for this module.
          </div>
        )}
      </div>
    </section>
  );
}

function FunctionSignatureCard({
  isOpen,
  onToggle,
  signature,
}: {
  isOpen: boolean;
  onToggle: () => void;
  signature: {
    body: string | null;
    isEntry: boolean;
    name: string;
    signature: string;
    visibility: string;
  };
}) {
  const source = isOpen && signature.body ? signature.body : signature.signature;

  return (
    <article className="rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] p-4">
      <button
        className="flex w-full min-w-0 items-center justify-between gap-3 text-left"
        onClick={onToggle}
        type="button"
      >
        <div className="flex min-w-0 items-center gap-2">
          <FileCode2 className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
          <h3 className="truncate text-sm font-semibold">{signature.name}</h3>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <Badge tone={visibilityTone(signature.visibility)}>
            {signature.visibility}
          </Badge>
          {signature.isEntry ? <Badge tone="entry">entry</Badge> : null}
          {signature.body ? (
            <ChevronDown
              className={cn(
                "size-4 text-muted-foreground transition-transform",
                isOpen && "rotate-180",
              )}
              aria-hidden="true"
            />
          ) : null}
        </div>
      </button>
      <pre className="mt-3 max-h-[420px] overflow-auto rounded-md bg-[var(--app-subtle)] p-3 text-xs leading-5 [font-family:'JetBrains_Mono','JetBrains_Mono_NL','JetBrains_Mono_NF',ui-monospace,SFMono-Regular,'SF_Mono',Menlo,Monaco,Consolas,'Liberation_Mono',monospace]">
        <code>
          <HighlightedMoveSignature source={source} />
        </code>
      </pre>
    </article>
  );
}

function functionKey(signature: { name: string; signature: string }) {
  return `${signature.name}-${signature.signature}`;
}

function HighlightedMoveSignature({ source }: { source: string }) {
  return (
    <>
      {tokenizeMoveSignature(source).map((token, index) => (
        <span
          className={cn(
            token.kind === "keyword" && "text-sky-300",
            token.kind === "ability" && "text-emerald-300",
            token.kind === "type" && "text-violet-300",
            token.kind === "number" && "text-amber-300",
            token.kind === "punctuation" && "text-muted-foreground",
            token.kind === "module" && "text-cyan-300",
            token.kind === "identifier" && "text-foreground",
            token.kind === "plain" && "text-foreground",
          )}
          key={`${token.value}-${index}`}
        >
          {token.value}
        </span>
      ))}
    </>
  );
}

type MoveSignatureToken = {
  kind:
    | "ability"
    | "identifier"
    | "keyword"
    | "module"
    | "number"
    | "plain"
    | "punctuation"
    | "type";
  value: string;
};

const MOVE_SIGNATURE_TOKEN_PATTERN =
  /(::|[A-Za-z_][A-Za-z0-9_]*|\d+|[{}()[\]<>,:;.=*&]|\s+|.)/g;
const MOVE_KEYWORDS = new Set([
  "acquires",
  "entry",
  "fun",
  "has",
  "friend",
  "mut",
  "native",
  "package",
  "public",
  "struct",
]);
const MOVE_ABILITIES = new Set(["copy", "drop", "key", "store"]);
const MOVE_PRIMITIVE_TYPES = new Set([
  "address",
  "bool",
  "signer",
  "u8",
  "u16",
  "u32",
  "u64",
  "u128",
  "u256",
  "vector",
]);

function tokenizeMoveSignature(source: string): MoveSignatureToken[] {
  return Array.from(source.matchAll(MOVE_SIGNATURE_TOKEN_PATTERN), (match) => {
    const value = match[0];

    if (/^\s+$/.test(value)) {
      return { kind: "plain", value };
    }

    if (MOVE_KEYWORDS.has(value)) {
      return { kind: "keyword", value };
    }

    if (MOVE_ABILITIES.has(value)) {
      return { kind: "ability", value };
    }

    if (MOVE_PRIMITIVE_TYPES.has(value)) {
      return { kind: "type", value };
    }

    if (/^\d+$/.test(value)) {
      return { kind: "number", value };
    }

    if (value === "::") {
      return { kind: "module", value };
    }

    if (/^[{}()[\]<>,:;.=*&]$/.test(value)) {
      return { kind: "punctuation", value };
    }

    if (/^[A-Za-z_][A-Za-z0-9_]*$/.test(value)) {
      return { kind: "identifier", value };
    }

    return { kind: "plain", value };
  });
}

function SurfaceSection({
  children,
  count,
  emptyText,
  title,
}: {
  children: React.ReactNode;
  count: number;
  emptyText: string;
  title: string;
}) {
  return (
    <section>
      <div className="mb-3 flex items-center justify-between gap-3">
        <h3 className="text-sm font-semibold text-foreground">{title}</h3>
        <span className="rounded bg-[var(--app-subtle)] px-2 py-0.5 text-xs text-muted-foreground">
          {count}
        </span>
      </div>
      {count ? (
        children
      ) : (
        <div className="rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-4 py-5 text-sm text-muted-foreground">
          {emptyText}
        </div>
      )}
    </section>
  );
}

function Badge({
  children,
  tone,
}: {
  children: string;
  tone: "ability" | "entry" | "private" | "public";
}) {
  return (
    <span
      className={cn(
        "rounded px-2 py-0.5 text-xs font-medium",
        tone === "ability" && "bg-sky-500/10 text-sky-300",
        tone === "public" && "bg-emerald-500/10 text-emerald-300",
        tone === "private" && "bg-muted text-muted-foreground",
        tone === "entry" && "bg-primary/15 text-primary",
      )}
    >
      {children}
    </span>
  );
}

function visibilityTone(visibility: string) {
  return visibility === "private" ? "private" : "public";
}
