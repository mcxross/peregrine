import type React from "react";
import {
  Boxes,
  Box,
  FileCode2,
  GitBranch,
  KeyRound,
  Link2,
  Lock,
  Network,
  ShieldAlert,
  UsersRound,
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import { ScrollArea } from "@/components/ui/scroll-area";
import type {
  AdminControlFinding,
  ExternalCallFinding,
  MoveFunctionSignature,
  MovePackage,
  ObjectOwnershipFinding,
  PublicPackageRelationship,
} from "@/features/empty-project/filesystem-tree";

export type SurfaceDetailKind =
  | "entry-functions"
  | "capabilities"
  | "shared-objects"
  | "address-owned"
  | "immutable-objects"
  | "wrapped-objects"
  | "party-objects"
  | "admin-controls"
  | "external-calls"
  | "package-internals";

type SurfaceDetailScreenProps = {
  detail: SurfaceDetailKind;
  movePackage: MovePackage | null;
};

const detailMeta = {
  "entry-functions": {
    icon: GitBranch,
    title: "Entry Functions",
    description: "Transaction-callable functions exposed by the active Move package.",
  },
  capabilities: {
    icon: KeyRound,
    title: "Capabilities",
    description: "Authority-bearing structs and the privileged functions they protect.",
  },
  "shared-objects": {
    icon: Boxes,
    title: "Shared Objects",
    description: "Key objects made globally accessible through Sui shared-object APIs.",
  },
  "address-owned": {
    icon: Box,
    title: "Address-Owned Objects",
    description: "Key objects transferred to, or returned for, transaction callers.",
  },
  "immutable-objects": {
    icon: Lock,
    title: "Immutable Objects",
    description: "Objects frozen into immutable state after creation.",
  },
  "wrapped-objects": {
    icon: Boxes,
    title: "Wrapped Objects",
    description: "Key objects stored inside other key objects.",
  },
  "party-objects": {
    icon: UsersRound,
    title: "Party Objects",
    description: "Objects moved through party ownership and transfer APIs.",
  },
  "admin-controls": {
    icon: ShieldAlert,
    title: "Admin Controls",
    description: "Privileged transaction-callable functions and their apparent guards.",
  },
  "external-calls": {
    icon: Network,
    title: "External Calls",
    description: "Calls from active package modules into external modules.",
  },
  "package-internals": {
    icon: Link2,
    title: "Package Internals",
    description: "Observed uses of public(package) functions inside the active package.",
  },
} satisfies Record<SurfaceDetailKind, {
  icon: typeof GitBranch;
  title: string;
  description: string;
}>;

export function SurfaceDetailScreen({
  detail,
  movePackage,
}: SurfaceDetailScreenProps) {
  const meta = detailMeta[detail];
  const Icon = meta.icon;

  return (
    <section className="grid h-full min-h-0 grid-rows-[auto_1fr] bg-[var(--app-window)]">
      <header className="border-b border-[color:var(--app-border)] px-6 py-5">
        <div className="flex items-start gap-3">
          <span className="inline-flex size-9 items-center justify-center rounded-md bg-[var(--app-elevated)] text-muted-foreground">
            <Icon className="size-5" aria-hidden="true" />
          </span>
          <div className="min-w-0">
            <h1 className="text-xl font-semibold tracking-tight">{meta.title}</h1>
            <p className="mt-1 max-w-2xl text-sm text-muted-foreground">{meta.description}</p>
          </div>
        </div>
      </header>

      <ScrollArea className="min-h-0">
        <div className="grid gap-3 p-6">
          {movePackage ? renderDetail(detail, movePackage) : <EmptyState label="No active Move package selected." />}
        </div>
      </ScrollArea>
    </section>
  );
}

function renderDetail(detail: SurfaceDetailKind, movePackage: MovePackage) {
  switch (detail) {
    case "entry-functions":
      return <EntryFunctionsDetail movePackage={movePackage} />;
    case "capabilities":
      return (
        <FindingList
          emptyLabel="No capability-like structs found."
          items={movePackage.surface.capabilityFindings.filter((finding) => finding.confidence !== "low")}
          renderItem={(finding) => (
            <FindingCard
              key={finding.qualifiedName}
              badge={finding.confidence}
              evidence={finding.evidence}
              title={finding.qualifiedName}
              subtitle={finding.protectedFunctions.length ? "Protects privileged functions" : "Capability candidate"}
            >
              <InlineList label="Protected functions" values={finding.protectedFunctions} />
            </FindingCard>
          )}
        />
      );
    case "shared-objects":
      return <OwnershipDetail movePackage={movePackage} kind="shared" />;
    case "address-owned":
      return <OwnershipDetail movePackage={movePackage} kind="addressOwned" />;
    case "immutable-objects":
      return <OwnershipDetail movePackage={movePackage} kind="immutable" />;
    case "wrapped-objects":
      return <OwnershipDetail movePackage={movePackage} kind="wrapped" />;
    case "party-objects":
      return <OwnershipDetail movePackage={movePackage} kind="party" />;
    case "admin-controls":
      return <AdminControlsDetail findings={movePackage.surface.adminControlFindings} />;
    case "external-calls":
      return <ExternalCallsDetail findings={movePackage.surface.externalCallFindings} />;
    case "package-internals":
      return <PackageInternalsDetail relationships={movePackage.surface.publicPackageRelationships} />;
  }
}

function EntryFunctionsDetail({ movePackage }: { movePackage: MovePackage }) {
  const functions = movePackage.modules.flatMap((module) =>
    module.functions
      .filter((functionSignature) => functionSignature.isTransactionCallable)
      .map((functionSignature) => ({ functionSignature, moduleName: module.name })),
  );

  return (
    <FindingList
      emptyLabel="No transaction-callable functions found."
      items={functions}
      renderItem={({ functionSignature, moduleName }) => (
        <FunctionCard
          key={`${moduleName}::${functionSignature.name}`}
          functionSignature={functionSignature}
          moduleName={moduleName}
        />
      )}
    />
  );
}

function OwnershipDetail({
  kind,
  movePackage,
}: {
  kind: ObjectOwnershipFinding["ownershipKind"];
  movePackage: MovePackage;
}) {
  const findings = movePackage.surface.objectOwnershipFindings.filter(
    (finding) => finding.ownershipKind === kind,
  );

  if (kind === "shared" && findings.length === 0 && movePackage.surface.sharedObjectStructs.length) {
    return (
      <div className="grid gap-3">
        {movePackage.surface.sharedObjectStructs.map((qualifiedName) => (
          <FindingCard
            badge="medium"
            evidence={["struct has key ability and appears in shared-object context"]}
            key={qualifiedName}
            title={qualifiedName}
            subtitle="Shared object candidate"
          />
        ))}
      </div>
    );
  }

  return (
    <FindingList
      emptyLabel="No matching object ownership findings found."
      items={findings}
      renderItem={(finding) => (
        <FindingCard
          key={`${finding.ownershipKind}:${finding.qualifiedName}`}
          badge={finding.confidence}
          evidence={finding.evidence}
          title={finding.qualifiedName}
          subtitle={ownershipSubtitle(finding.ownershipKind)}
        >
          <InlineList label="Related functions" values={finding.relatedFunctions} />
          <InlineList label="Wrapped types" values={finding.wrappedTypes} />
        </FindingCard>
      )}
    />
  );
}

function AdminControlsDetail({ findings }: { findings: AdminControlFinding[] }) {
  return (
    <FindingList
      emptyLabel="No admin-control findings found."
      items={findings}
      renderItem={(finding) => (
        <FindingCard
          key={finding.qualifiedName}
          badge={finding.confidence}
          evidence={finding.evidence}
          title={finding.qualifiedName}
          subtitle="Privileged transaction-callable function"
        >
          <InlineList label="Guarding types" values={finding.guardingTypes} />
        </FindingCard>
      )}
    />
  );
}

function ExternalCallsDetail({ findings }: { findings: ExternalCallFinding[] }) {
  return (
    <FindingList
      emptyLabel="No external calls found."
      items={findings}
      renderItem={(finding) => (
        <Card className="gap-0 rounded-md p-4" key={`${finding.callerModule}:${finding.callerFunction}:${finding.target}`}>
          <div className="flex min-w-0 items-start gap-3">
            <Network className="mt-1 size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
            <div className="min-w-0 flex-1">
              <h2 className="truncate text-sm font-semibold">
                {finding.callerModule}::{finding.callerFunction}
              </h2>
              <p className="mt-1 text-sm text-muted-foreground">calls external target</p>
              <CodeLine>{finding.target}</CodeLine>
            </div>
          </div>
        </Card>
      )}
    />
  );
}

function PackageInternalsDetail({
  relationships,
}: {
  relationships: PublicPackageRelationship[];
}) {
  return (
    <FindingList
      emptyLabel="No public(package) relationships found."
      items={relationships}
      renderItem={(relationship) => (
        <Card
          className="gap-0 rounded-md p-4"
          key={`${relationship.sourceModule}:${relationship.sourceFunction}:${relationship.targetModule}:${relationship.targetFunction}`}
        >
          <div className="flex min-w-0 items-start gap-3">
            <Link2 className="mt-1 size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
            <div className="min-w-0 flex-1">
              <h2 className="truncate text-sm font-semibold">
                {relationship.sourceModule}::{relationship.sourceFunction}
              </h2>
              <p className="mt-1 text-sm text-muted-foreground">uses package-scoped function</p>
              <CodeLine>
                {relationship.targetModule}::{relationship.targetFunction}
              </CodeLine>
            </div>
          </div>
        </Card>
      )}
    />
  );
}

function FunctionCard({
  functionSignature,
  moduleName,
}: {
  functionSignature: MoveFunctionSignature;
  moduleName: string;
}) {
  return (
    <Card className="gap-0 rounded-md p-4">
      <div className="flex min-w-0 items-start gap-3">
        <FileCode2 className="mt-1 size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
        <div className="min-w-0 flex-1">
          <div className="flex min-w-0 items-center gap-2">
            <h2 className="truncate text-sm font-semibold">
              {moduleName}::{functionSignature.name}
            </h2>
            <Badge className="rounded bg-primary/10 px-2 py-0.5 text-[11px] text-primary" variant="secondary">
              {functionSignature.isEntry ? "entry" : functionSignature.visibility}
            </Badge>
          </div>
          <CodeLine>{functionSignature.signature}</CodeLine>
        </div>
      </div>
    </Card>
  );
}

function FindingCard({
  badge,
  children,
  evidence,
  subtitle,
  title,
}: {
  badge: string;
  children?: React.ReactNode;
  evidence: string[];
  subtitle: string;
  title: string;
}) {
  return (
    <Card className="gap-0 rounded-md p-4">
      <div className="flex min-w-0 items-start justify-between gap-3">
        <div className="min-w-0">
          <h2 className="truncate text-sm font-semibold">{title}</h2>
          <p className="mt-1 text-sm text-muted-foreground">{subtitle}</p>
        </div>
        <Badge className="rounded bg-[var(--app-subtle)] px-2 py-0.5 text-[11px]" variant="secondary">
          {badge}
        </Badge>
      </div>
      <InlineList label="Evidence" values={evidence} />
      {children}
    </Card>
  );
}

function InlineList({ label, values }: { label: string; values: string[] }) {
  if (!values.length) {
    return null;
  }

  return (
    <div className="mt-3">
      <div className="text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">{label}</div>
      <div className="mt-2 flex flex-wrap gap-1.5">
        {values.map((value) => (
          <span
            className="rounded bg-[var(--app-subtle)] px-2 py-1 font-mono text-xs text-muted-foreground"
            key={value}
          >
            {value}
          </span>
        ))}
      </div>
    </div>
  );
}

function FindingList<TItem>({
  emptyLabel,
  items,
  renderItem,
}: {
  emptyLabel: string;
  items: TItem[];
  renderItem: (item: TItem) => React.ReactNode;
}) {
  if (!items.length) {
    return <EmptyState label={emptyLabel} />;
  }

  return <div className="grid gap-3">{items.map(renderItem)}</div>;
}

function CodeLine({ children }: { children: React.ReactNode }) {
  return (
    <pre className="mt-3 overflow-x-auto rounded-md bg-[var(--app-subtle)] px-3 py-2 font-mono text-xs leading-5 text-foreground">
      {children}
    </pre>
  );
}

function EmptyState({ label }: { label: string }) {
  return (
    <div className="flex min-h-64 items-center justify-center rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-6 text-center text-sm text-muted-foreground">
      {label}
    </div>
  );
}

function ownershipSubtitle(kind: ObjectOwnershipFinding["ownershipKind"]) {
  switch (kind) {
    case "shared":
      return "Shared object ownership";
    case "addressOwned":
      return "Address-owned object ownership";
    case "immutable":
      return "Immutable object ownership";
    case "wrapped":
      return "Wrapped object relationship";
    case "party":
      return "Party object ownership";
  }
}
