import {
  buildMovePackage,
  defaultProjectMetadata,
  loadFilePreview,
  loadPackageTree,
  loadProjectMetadata,
  projectMoveCoverageScriptPath,
  projectMoveTestScriptPath,
  runFormalVerification,
  runMovyFuzz,
  runSecurityCommand,
  runSecurityScript,
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
  | "skipped"
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
    enabled: true,
    id: "tests",
    label: "Tests",
  },
  {
    command: "sui move test --coverage",
    enabled: true,
    id: "coverage",
    label: "Coverage",
  },
  {
    command: "movy fuzz public-functions",
    enabled: true,
    id: "fuzzing",
    label: "Fuzzing",
  },
  {
    command: "bundled sui-prover --path <package> --modules <module>",
    enabled: true,
    id: "formal",
    label: "Formal",
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
  coverage: runCoverageAssessment,
  formal: runFormalAssessment,
  fuzzing: runFuzzAssessment,
  tests: runTestsAssessment,
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

async function runTestsAssessment(context: PackageLoadAssessmentCommandRunnerContext) {
  const metadata = await loadProjectMetadata(context.packageTree.rootPath).catch((error) => {
    console.warn("Could not load project metadata; running default Move tests.", error);
    return defaultProjectMetadata();
  });
  const testScriptPath = await existingProjectScriptPath({
    label: "test",
    movePackage: context.movePackage,
    packageTree: context.packageTree,
    scriptPath: projectMoveTestScriptPath(metadata, context.movePackage),
  });

  return runCommandAssessment(context, {
    command: testScriptPath ? `bash ${testScriptPath}` : "sui move test",
    metadata: testScriptPath
      ? [
          { label: "Mode", value: "Project script" },
          { label: "Default", value: "sui move test" },
        ]
      : undefined,
    runningText: "Running Move tests...",
    run: (logRun, onOutput) =>
      testScriptPath
        ? runSecurityScript(context.packageTree, context.movePackage.path, testScriptPath, {
            onOutput,
            streamId: logRun.id,
          })
        : runSecurityCommand(context.packageTree, context.movePackage.path, "move-test", {
            onOutput,
            streamId: logRun.id,
          }),
    summary: (output) =>
      output.status === 0
        ? "Move tests passed."
        : output.status == null
          ? "Move tests failed."
          : `Move tests failed ${output.status}.`,
  });
}

async function runCoverageAssessment(context: PackageLoadAssessmentCommandRunnerContext) {
  const metadata = await loadProjectMetadata(context.packageTree.rootPath).catch((error) => {
    console.warn("Could not load project metadata; running default Move coverage.", error);
    return defaultProjectMetadata();
  });
  const coverageScriptPath = await existingProjectScriptPath({
    label: "coverage",
    movePackage: context.movePackage,
    packageTree: context.packageTree,
    scriptPath: projectMoveCoverageScriptPath(metadata, context.movePackage),
  });
  const testScriptPath = coverageScriptPath
    ? null
    : await existingProjectScriptPath({
        label: "test coverage fallback",
        movePackage: context.movePackage,
        packageTree: context.packageTree,
        scriptPath: projectMoveTestScriptPath(metadata, context.movePackage),
      });
  const scriptPath = coverageScriptPath ?? testScriptPath;
  const scriptArgs = coverageScriptPath ? [] : testScriptPath ? ["--coverage"] : [];
  const command = scriptPath
    ? `bash ${scriptPath}${scriptArgs.length ? ` ${scriptArgs.join(" ")}` : ""}`
    : "sui move test --coverage";

  let assessment = await runCommandAssessment(context, {
    command,
    metadata: scriptPath
      ? [
          { label: "Mode", value: coverageScriptPath ? "Project coverage script" : "Project test script" },
          { label: "Default", value: "sui move test --coverage" },
          ...(scriptArgs.length ? [{ label: "Args", value: scriptArgs.join(" ") }] : []),
        ]
      : undefined,
    runningText: "Running tests with coverage...",
    run: (logRun, onOutput) =>
      scriptPath
        ? runSecurityScript(context.packageTree, context.movePackage.path, scriptPath, {
            args: scriptArgs,
            onOutput,
            streamId: logRun.id,
          })
        : runSecurityCommand(context.packageTree, context.movePackage.path, "move-coverage", {
            onOutput,
            streamId: logRun.id,
          }),
    summary: (output) =>
      output.status === 0
        ? "Coverage test run passed."
        : output.status == null
          ? "Coverage test run failed."
          : `Coverage test run failed ${output.status}.`,
  });

  const coverageStep = assessment.steps.find((step) => step.id === "coverage");

  if (scriptPath || coverageStep?.state !== "success") {
    return assessment;
  }

  const summaryCommand: PackageLoadAssessmentCommand = {
    ...context.command,
    command: "sui move coverage summary",
    label: "Coverage summary",
  };
  const summaryContext = {
    ...context,
    assessment,
    command: summaryCommand,
  };

  assessment = await runCommandAssessment(summaryContext, {
    command: "sui move coverage summary",
    preserveStepStart: true,
    runningText: "Reading coverage summary...",
    run: (logRun, onOutput) =>
      runSecurityCommand(context.packageTree, context.movePackage.path, "move-coverage-summary", {
        onOutput,
        streamId: logRun.id,
      }),
    summary: (output) =>
      output.status === 0
        ? "Coverage summary completed."
        : output.status == null
          ? "Coverage summary failed."
          : `Coverage summary failed ${output.status}.`,
  });

  return assessment;
}

async function existingProjectScriptPath({
  label,
  movePackage,
  packageTree,
  scriptPath,
}: {
  label: string;
  movePackage: MovePackage;
  packageTree: PackageTree;
  scriptPath: string | null;
}) {
  const trimmedScriptPath = scriptPath?.trim();

  if (!trimmedScriptPath) {
    return null;
  }

  try {
    await loadFilePreview(
      packageTree,
      packageRelativeScriptPath(movePackage.path, trimmedScriptPath),
      { includeHighlightedHtml: false },
    );

    return trimmedScriptPath;
  } catch (error) {
    console.warn(
      `Configured ${label} script ${trimmedScriptPath} was not found; falling back to the default command.`,
      error,
    );
    return null;
  }
}

function packageRelativeScriptPath(packagePath: string, scriptPath: string) {
  const normalizedScriptPath = scriptPath.replace(/^\/+/, "");
  const normalizedPackagePath = packagePath === "."
    ? ""
    : packagePath.replace(/^\/+|\/+$/g, "");

  return normalizedPackagePath
    ? `${normalizedPackagePath}/${normalizedScriptPath}`
    : normalizedScriptPath;
}

async function runFuzzAssessment(context: PackageLoadAssessmentCommandRunnerContext) {
  return runCommandAssessment(context, {
    command: "movy fuzz public-functions",
    metadata: [{ label: "Scope", value: "Public functions only" }],
    runningText: "Deploying package into Movy's executor...",
    run: (logRun, onOutput) =>
      runMovyFuzz(context.packageTree, context.movePackage.path, {
        onOutput,
        streamId: logRun.id,
      }),
    summary: (output) =>
      output.status === 0
        ? "Movy fuzzing passed."
        : output.status == null
          ? "Movy fuzzing failed."
          : `Movy fuzzing failed ${output.status}.`,
  });
}

async function runFormalAssessment(context: PackageLoadAssessmentCommandRunnerContext) {
  if (context.movePackage.modules.length === 0) {
    const finishedAt = new Date();
    const detail = "No parseable Move modules were found for the active package.";
    const assessment = updateAssessmentStep(context.assessment, "formal", {
      caption: "No formal targets",
      detail,
      finishedAt,
      state: "skipped",
      value: "Skipped",
    });

    publishAssessment(assessment, context.onAssessmentChange, context.isCurrent);
    return assessment;
  }

  return runCommandAssessment(context, {
    command: "bundled sui-prover --path <package> --modules <module>",
    metadata: [
      { label: "Mode", value: "Bundled Sui Prover" },
      { label: "Modules", value: String(context.movePackage.modules.length) },
      { label: "Timeout", value: "45 seconds per module" },
    ],
    runningText: "Running bundled Sui Prover...",
    run: async (logRun, onOutput) => {
      const aggregateOutput: CommandOutput = {
        status: 0,
        stderr: "",
        stdout: "",
      };

      for (const moveModule of context.movePackage.modules) {
        const output = await runFormalVerification(
          context.packageTree,
          context.movePackage.path,
          moveModule.filePath,
          moveModule.name,
          {
            onOutput: (partialOutput) => {
              onOutput({
                status: partialOutput.status,
                stderr: aggregateOutput.stderr + partialOutput.stderr,
                stdout: aggregateOutput.stdout + partialOutput.stdout,
              });
            },
            streamId: logRun.id,
            timeoutSeconds: 45,
          },
        );

        aggregateOutput.stdout += output.stdout;
        aggregateOutput.stderr += output.stderr;

        if (output.status !== 0) {
          aggregateOutput.status = output.status ?? 1;
        }
      }

      return aggregateOutput;
    },
    summary: (output) =>
      output.status === 0
        ? "Formal verification passed."
        : output.status == null
          ? "Formal verification failed."
          : `Formal verification failed ${output.status}.`,
  });
}

type RunAssessmentOptions = {
  command: string;
  metadata?: { label: string; value: string }[];
  preserveStepStart?: boolean;
  runningText: string;
  run: (
    logRun: BuildLogRun,
    onOutput: (output: CommandOutput) => void,
  ) => Promise<CommandOutput>;
  summary: (output: CommandOutput) => string;
};

async function runCommandAssessment(
  context: PackageLoadAssessmentCommandRunnerContext,
  options: RunAssessmentOptions,
) {
  const startedAt = new Date();
  const logRun = assessmentLogRun({
    assessment: context.assessment,
    command: {
      ...context.command,
      command: options.command,
    },
    movePackage: context.movePackage,
    packageTree: context.packageTree,
    startedAt,
    state: "running",
  });
  let assessment = updateAssessmentStep(context.assessment, context.command.id, {
    caption: options.command,
    command: options.command,
    detail: options.runningText,
    startedAt: options.preserveStepStart
      ? context.assessment.steps.find((step) => step.id === context.command.id)?.startedAt ?? startedAt
      : startedAt,
    state: "running",
    value: "Running",
  });

  publishAssessment(assessment, context.onAssessmentChange, context.isCurrent);
  publishLog(
    {
      ...logRun,
      metadata: [
        { label: "Step", value: context.command.label },
        { label: "Mode", value: "Package load" },
        ...(options.metadata ?? []),
      ],
      runningText: options.runningText,
    },
    context.onCommandLog,
    context.isCurrent,
    { open: false, reset: context.command.id === "build" },
  );

  try {
    const output = await options.run(logRun, (streamedOutput) => {
      publishLog(
        {
          ...logRun,
          metadata: [
            { label: "Step", value: context.command.label },
            { label: "Mode", value: "Package load" },
            ...(options.metadata ?? []),
          ],
          output: streamedOutput,
          runningText: options.runningText,
        },
        context.onCommandLog,
        context.isCurrent,
        { open: false },
      );
    });
    const finishedAt = new Date();
    const succeeded = output.status === 0;
    const summary = options.summary(output);

    assessment = updateAssessmentStep(assessment, context.command.id, {
      caption: succeeded ? "Command completed" : "Command failed",
      detail: summary,
      finishedAt,
      output,
      state: succeeded ? "success" : "error",
      value: succeeded ? "Pass" : "Fail",
    });
    publishAssessment(assessment, context.onAssessmentChange, context.isCurrent);
    publishLog(
      {
        ...logRun,
        finishedAt,
        metadata: [
          { label: "Step", value: context.command.label },
          { label: "Mode", value: "Package load" },
          ...(options.metadata ?? []),
          { label: "Summary", value: summary },
        ],
        output,
        state: succeeded ? "success" : "error",
      },
      context.onCommandLog,
      context.isCurrent,
      { open: !succeeded },
    );

    return assessment;
  } catch (error) {
    const finishedAt = new Date();
    const message = getLoadAssessmentErrorMessage(error);

    assessment = updateAssessmentStep(assessment, context.command.id, {
      caption: "Command failed",
      detail: message,
      finishedAt,
      state: "error",
      value: "Fail",
    });
    publishAssessment(assessment, context.onAssessmentChange, context.isCurrent);
    publishLog(
      {
        ...logRun,
        error: message,
        finishedAt,
        metadata: [
          { label: "Step", value: context.command.label },
          { label: "Mode", value: "Package load" },
          ...(options.metadata ?? []),
          { label: "Summary", value: "Command failed before it could complete" },
        ],
        state: "error",
      },
      context.onCommandLog,
      context.isCurrent,
      { open: true },
    );

    return assessment;
  }
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
    value: command.enabled ? "Pending" : isRisk ? "Pending" : "Skipped",
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
