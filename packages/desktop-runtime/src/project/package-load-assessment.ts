import {
  buildMovePackage,
  loadPackageTree,
  type CommandOutput,
  type MovePackage,
  type PackageTree,
} from "./filesystem-tree";
import type {
  BuildLogRun,
  BuildLogUpdateOptions,
} from "./build-log-types";

export type PackageLoadAssessmentCommandId =
  | "build"
  | "tests"
  | "coverage"
  | "fuzzing"
  | "formal"
  | "risk";

export type PackageLoadAssessmentState =
  | "idle"
  | "running"
  | "success"
  | "attention"
  | "error"
  | "muted";

export type PackageLoadAssessmentCommand = {
  command: string | null;
  enabled: boolean;
  id: PackageLoadAssessmentCommandId;
  label: string;
  mutedCaption?: string;
};

export type PackageLoadAssessmentStep = {
  caption: string;
  command: string | null;
  detail: string | null;
  enabled: boolean;
  finishedAt: Date | null;
  id: PackageLoadAssessmentCommandId;
  label: string;
  output: CommandOutput | null;
  startedAt: Date | null;
  state: PackageLoadAssessmentState;
  value: string;
};

export type PackageLoadAssessment = {
  finishedAt: Date | null;
  key: string;
  packageName: string;
  packagePath: string;
  startedAt: Date;
  steps: PackageLoadAssessmentStep[];
};

export type PackageLoadAssessmentRunnerOptions = {
  isCurrent?: () => boolean;
  movePackage: MovePackage;
  onAssessmentChange: (assessment: PackageLoadAssessment) => void;
  onCommandLog: (run: BuildLogRun, options?: BuildLogUpdateOptions) => void;
  onProjectSelected: (packageTree: PackageTree) => void;
  packageTree: PackageTree;
};

type PackageLoadAssessmentCommandRunnerContext = {
  assessment: PackageLoadAssessment;
  command: PackageLoadAssessmentCommand;
  isCurrent: () => boolean;
  movePackage: MovePackage;
  onAssessmentChange: (assessment: PackageLoadAssessment) => void;
  onCommandLog: (run: BuildLogRun, options?: BuildLogUpdateOptions) => void;
  onProjectSelected: (packageTree: PackageTree) => void;
  packageTree: PackageTree;
};

type PackageLoadAssessmentCommandRunner = (
  context: PackageLoadAssessmentCommandRunnerContext,
) => Promise<PackageLoadAssessment>;

export const packageLoadAssessmentCommands: PackageLoadAssessmentCommand[] = [
  {
    command: "sui move build",
    enabled: true,
    id: "build",
    label: "Build",
  },
  {
    command: "sui move test",
    enabled: false,
    id: "tests",
    label: "Tests",
    mutedCaption: "Not enabled",
  },
  {
    command: "sui move test --coverage",
    enabled: false,
    id: "coverage",
    label: "Coverage",
    mutedCaption: "Not enabled",
  },
  {
    command: "sui move test --rand-num-iters 256",
    enabled: false,
    id: "fuzzing",
    label: "Fuzzing",
    mutedCaption: "Not enabled",
  },
  {
    command: "sui-prover",
    enabled: false,
    id: "formal",
    label: "Formal",
    mutedCaption: "Not enabled",
  },
  {
    command: null,
    enabled: false,
    id: "risk",
    label: "Risk",
    mutedCaption: "No score",
  },
];

const packageLoadAssessmentRunners: Partial<Record<
  PackageLoadAssessmentCommandId,
  PackageLoadAssessmentCommandRunner
>> = {
  build: runBuildAssessment,
};

export function packageLoadAssessmentKey(
  packageTree: PackageTree,
  movePackage: MovePackage,
) {
  return `${packageTree.rootPath}::${movePackage.manifestPath}`;
}

export function createPackageLoadAssessment({
  movePackage,
  packageTree,
  startedAt = new Date(),
}: {
  movePackage: MovePackage;
  packageTree: PackageTree;
  startedAt?: Date;
}): PackageLoadAssessment {
  return {
    finishedAt: null,
    key: packageLoadAssessmentKey(packageTree, movePackage),
    packageName: movePackage.name,
    packagePath: movePackage.path || ".",
    startedAt,
    steps: packageLoadAssessmentCommands.map(createAssessmentStep),
  };
}

export async function runPackageLoadAssessment({
  isCurrent = () => true,
  movePackage,
  onAssessmentChange,
  onCommandLog,
  onProjectSelected,
  packageTree,
}: PackageLoadAssessmentRunnerOptions) {
  let assessment = createPackageLoadAssessment({ movePackage, packageTree });

  publishAssessment(assessment, onAssessmentChange, isCurrent);

  for (const command of packageLoadAssessmentCommands) {
    if (!command.enabled) {
      continue;
    }

    const runCommand = packageLoadAssessmentRunners[command.id];

    if (runCommand) {
      assessment = await runCommand({
        assessment,
        command,
        isCurrent,
        movePackage,
        onAssessmentChange,
        onCommandLog,
        onProjectSelected,
        packageTree,
      });
    }
  }

  assessment = {
    ...assessment,
    finishedAt: new Date(),
  };
  publishAssessment(assessment, onAssessmentChange, isCurrent);
}

async function runBuildAssessment({
  assessment,
  command,
  isCurrent,
  movePackage,
  onAssessmentChange,
  onCommandLog,
  onProjectSelected,
  packageTree,
}: PackageLoadAssessmentCommandRunnerContext) {
  const startedAt = new Date();
  const logRun = assessmentLogRun({
    assessment,
    command,
    movePackage,
    packageTree,
    startedAt,
    state: "running",
  });

  assessment = updateAssessmentStep(assessment, command.id, {
    caption: command.command ?? "",
    startedAt,
    state: "running",
    value: "Run",
  });
  publishAssessment(assessment, onAssessmentChange, isCurrent);
  publishLog(logRun, onCommandLog, isCurrent, { open: false, reset: true });

  try {
    const output = await buildMovePackage(packageTree, movePackage.path, {
      streamId: logRun.id,
      onOutput: (streamedOutput) => {
        publishLog(
          {
            ...logRun,
            output: streamedOutput,
          },
          onCommandLog,
          isCurrent,
          { open: false },
        );
      },
    });
    const finishedAt = new Date();
    const succeeded = output.status === 0;
    let finalState: PackageLoadAssessmentState = succeeded ? "success" : "error";
    let value = succeeded ? "Pass" : "Fail";
    let caption = succeeded ? "Summaries refreshed" : "Build failed";
    let summary = succeeded
      ? "Build passed"
      : output.status == null
        ? "Build failed"
        : `Build failed ${output.status}`;

    if (succeeded && isCurrent()) {
      try {
        const refreshedPackageTree = await loadPackageTree(packageTree.rootPath);
        const activePackageManifestPath = refreshedPackageTree.movePackages.some(
          (candidate) => candidate.manifestPath === movePackage.manifestPath,
        )
          ? movePackage.manifestPath
          : refreshedPackageTree.movePackages[0]?.manifestPath ?? null;

        if (refreshedPackageTree.dependencyGraph.summaryPath) {
          caption = "Summaries refreshed";
        } else {
          caption = "Build passed";
          summary = "Build passed. Package summaries were not found after rescanning, so dependency graph detail may be limited.";
        }

        if (isCurrent()) {
          onProjectSelected({
            ...refreshedPackageTree,
            activePackageManifestPath,
          });
        }
      } catch (error) {
        finalState = "attention";
        value = "Review";
        caption = "Rescan failed";
        summary = `Build passed, but Peregrine could not rescan the package: ${getLoadAssessmentErrorMessage(error)}`;
      }
    }

    assessment = updateAssessmentStep(assessment, command.id, {
      caption,
      detail: summary,
      finishedAt,
      output,
      state: finalState,
      value,
    });
    publishAssessment(assessment, onAssessmentChange, isCurrent);
    publishLog(
      {
        ...logRun,
        finishedAt,
        metadata: [
          { label: "Step", value: command.label },
          { label: "Mode", value: "Package load" },
          { label: "Summary", value: summary },
        ],
        output,
        state: succeeded ? "success" : "error",
      },
      onCommandLog,
      isCurrent,
      { open: !succeeded || finalState === "attention" },
    );
  } catch (error) {
    const finishedAt = new Date();
    const message = getLoadAssessmentErrorMessage(error);

    assessment = updateAssessmentStep(assessment, command.id, {
      caption: "Build failed",
      detail: message,
      finishedAt,
      state: "error",
      value: "Fail",
    });
    publishAssessment(assessment, onAssessmentChange, isCurrent);
    publishLog(
      {
        ...logRun,
        error: message,
        finishedAt,
        metadata: [
          { label: "Step", value: command.label },
          { label: "Mode", value: "Package load" },
          { label: "Summary", value: "Build failed before it could complete" },
        ],
        state: "error",
      },
      onCommandLog,
      isCurrent,
      { open: true },
    );
  }

  return assessment;
}

function createAssessmentStep(command: PackageLoadAssessmentCommand): PackageLoadAssessmentStep {
  const isRisk = command.id === "risk";

  return {
    caption: command.enabled
      ? command.command ?? ""
      : command.mutedCaption ?? "Not enabled",
    command: command.command,
    detail: null,
    enabled: command.enabled,
    finishedAt: null,
    id: command.id,
    label: command.label,
    output: null,
    startedAt: null,
    state: command.enabled ? "idle" : "muted",
    value: command.enabled ? "Queued" : isRisk ? "Pending" : "Locked",
  };
}

function updateAssessmentStep(
  assessment: PackageLoadAssessment,
  stepId: PackageLoadAssessmentCommandId,
  update: Partial<PackageLoadAssessmentStep>,
) {
  return {
    ...assessment,
    steps: assessment.steps.map((step) =>
      step.id === stepId
        ? {
            ...step,
            ...update,
          }
        : step,
    ),
  };
}

function assessmentLogRun({
  assessment,
  command,
  movePackage,
  packageTree,
  startedAt,
  state,
}: {
  assessment: PackageLoadAssessment;
  command: PackageLoadAssessmentCommand;
  movePackage: MovePackage;
  packageTree: PackageTree;
  startedAt: Date;
  state: BuildLogRun["state"];
}): BuildLogRun {
  return {
    canRerun: false,
    command: command.command ?? command.label,
    emptyText: "Package load command finished without output.",
    error: null,
    finishedAt: null,
    id: hashRunId(`${assessment.key}:${command.id}:${startedAt.getTime()}`),
    metadata: [
      { label: "Step", value: command.label },
      { label: "Mode", value: "Package load" },
    ],
    output: null,
    packageName: movePackage.name,
    packagePath: movePackage.path || ".",
    runningText: `Running ${command.label.toLowerCase()} during package load...`,
    startedAt,
    state,
    title: "Package load",
    workingDirectory: packagePathLabel(movePackage, packageTree),
  };
}

function publishAssessment(
  assessment: PackageLoadAssessment,
  onAssessmentChange: (assessment: PackageLoadAssessment) => void,
  isCurrent: () => boolean,
) {
  if (isCurrent()) {
    onAssessmentChange(assessment);
  }
}

function publishLog(
  run: BuildLogRun,
  onCommandLog: (run: BuildLogRun, options?: BuildLogUpdateOptions) => void,
  isCurrent: () => boolean,
  options?: BuildLogUpdateOptions,
) {
  if (isCurrent()) {
    onCommandLog(run, options);
  }
}

function packagePathLabel(movePackage: MovePackage, packageTree: PackageTree) {
  if (!movePackage.path || movePackage.path === ".") {
    return packageTree.rootPath;
  }

  if (movePackage.path.startsWith("/")) {
    return movePackage.path;
  }

  return `${packageTree.rootPath}/${movePackage.path}`;
}

function hashRunId(value: string) {
  return value.split("").reduce((hash, character) => {
    return (hash * 31 + character.charCodeAt(0)) >>> 0;
  }, 11);
}

function getLoadAssessmentErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  return typeof error === "string" ? error : "Package load command failed.";
}
