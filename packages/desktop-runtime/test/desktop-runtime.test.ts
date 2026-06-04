import { describe, expect, test } from "bun:test";

import {
  findModuleByPath,
  sameSourcePath,
} from "../src/project/source-paths";
import {
  activeManifestPathForRecentProject,
  recentProjectFromPackageTree,
} from "../src/project/recent-project-store";
import {
  defaultProjectMetadata,
  projectMoveCoverageScriptPath,
  projectMoveTestScriptPath,
  projectPackageConfigKey,
  type ProjectMetadata,
} from "../src/project/filesystem-tree";
import {
  createPackageLoadAssessment,
} from "../src/project/package-load-assessment";
import {
  normalizePackageTree,
  type PackageTreeWire,
} from "../src/project/move-package-insights";
import {
  tokenizeMoveSignature,
} from "../src/module-signature/move-signature";
import {
  applyMoveAnalyzerTextEdits,
} from "../src/lsp/workspace-edit";
import {
  normalizeMoveAnalyzerLocations,
  normalizeMoveAnalyzerWorkspaceEdit,
} from "../src/lsp/normalizers";
import type { MoveAnalyzerTextEdit } from "../src/lsp/types";
import {
  ensurePrimaryAgentThreadState,
  formatAgentThreadName,
  syncAgentStudioStateWithServerThread,
} from "../src/agents/agent-server-threads";
import {
  agentStudioStateToProjectMetadata,
  loadAgentStudioStateFromProjectMetadata,
} from "../src/agents/agent-workflow-store";
import {
  mapAgentServerNotificationToRunEvents,
} from "../src/agents/agent-runner";
import type { ServerNotification } from "../../../crates/peregrine-app-server-protocol/schema/typescript";
import type { Thread } from "../../../crates/peregrine-app-server-protocol/schema/typescript/v2";

function packageTreeWire(): PackageTreeWire {
  return {
    activePackageManifestPath: "Move.toml",
    rootPath: "/tmp/demo",
    rootName: "demo",
    isDetailed: true,
    paths: ["Move.toml", "sources/main.move"],
    dependencyGraph: {
      root: "demo",
      nodes: [],
      edges: [],
      summaryPath: null,
    },
    callGraph: {
      nodes: [],
      edges: [],
      unresolvedCalls: [],
    },
    typeGraph: {
      nodes: [],
      edges: [],
      unresolvedTypes: [],
    },
    stateAccessGraph: {
      nodes: [],
      edges: [],
      unresolvedAccesses: [],
    },
    loadReport: {
      stages: [],
      capabilities: {},
      analysisReports: {},
    },
    movePackages: [
      {
        name: "demo",
        path: ".",
        manifestPath: "Move.toml",
        hasSourceFiles: true,
        hasSourceModules: true,
        sourceFileCount: 1,
        modules: [
          {
            name: "main",
            address: "0x0",
            filePath: "sources/main.move",
            attributes: [],
            structs: [],
            functions: [],
          },
        ],
      },
    ],
  };
}

describe("desktop runtime project models", () => {
  test("normalizes package trees with empty insight defaults", () => {
    const tree = normalizePackageTree(packageTreeWire());

    expect(tree.movePackages[0].surface.entryFunctionCount).toBe(0);
    expect(tree.movePackages[0].insights?.scannerReport.tests.unitTestCount).toBe(0);
  });

  test("initializes auto validation workflow steps as pending", () => {
    const tree = normalizePackageTree(packageTreeWire());
    const assessment = createPackageLoadAssessment({
      movePackage: tree.movePackages[0],
      packageTree: tree,
    });
    const workflowSteps = assessment.steps.filter((step) => step.id !== "risk");

    expect(workflowSteps.map((step) => step.id)).toEqual([
      "build",
      "tests",
      "coverage",
      "fuzzing",
      "formal",
    ]);
    expect(workflowSteps.every((step) => step.enabled)).toBe(true);
    expect(workflowSteps.every((step) => step.state === "idle")).toBe(true);
    expect(workflowSteps.every((step) => step.value === "Pending")).toBe(true);
  });

  test("resolves configured project command scripts by package key", () => {
    const tree = normalizePackageTree(packageTreeWire());
    const movePackage = tree.movePackages[0];
    const packageKey = projectPackageConfigKey(movePackage);
    const metadata: ProjectMetadata = {
      ...defaultProjectMetadata(),
      packageConfigs: {
        [packageKey]: {
          commands: {
            moveCoverageScriptPath: "scripts/coverage.sh",
            moveTestScriptPath: "scripts/test.sh",
          },
        },
      },
    };

    expect(projectMoveTestScriptPath(metadata, movePackage)).toBe("scripts/test.sh");
    expect(projectMoveCoverageScriptPath(metadata, movePackage)).toBe("scripts/coverage.sh");
  });

  test("derives and restores recent project package identity", () => {
    const tree = normalizePackageTree(packageTreeWire());
    const recent = recentProjectFromPackageTree(tree);

    expect(recent.activePackageManifestPath).toBe("Move.toml");
    expect(activeManifestPathForRecentProject(tree, recent)).toBe("Move.toml");
  });
});

describe("desktop runtime source helpers", () => {
  test("matches source paths across package-relative forms", () => {
    expect(sameSourcePath("sources/main.move", "/tmp/demo/sources/main.move", "/tmp/demo")).toBe(true);

    const tree = normalizePackageTree(packageTreeWire());
    expect(findModuleByPath(tree.movePackages, "/tmp/demo/sources/main.move", tree.movePackages[0])?.moveModule.name).toBe("main");
  });

  test("tokenizes Move signatures by semantic token kind", () => {
    const tokens = tokenizeMoveSignature("public entry fun mint<T: store>(cap: &mut Cap)");

    expect(tokens.some((token) => token.kind === "keyword" && token.value === "public")).toBe(true);
    expect(tokens.some((token) => token.kind === "ability" && token.value === "store")).toBe(true);
    expect(tokens.some((token) => token.kind === "punctuation" && token.value === "&")).toBe(true);
  });
});

describe("desktop runtime LSP helpers", () => {
  test("applies Move Analyzer text edits without offset drift", () => {
    const edits: MoveAnalyzerTextEdit[] = [
      {
        range: {
          start: { line: 0, character: 4 },
          end: { line: 0, character: 7 },
        },
        newText: "bar",
      },
      {
        range: {
          start: { line: 1, character: 0 },
          end: { line: 1, character: 3 },
        },
        newText: "baz",
      },
    ];

    expect(applyMoveAnalyzerTextEdits("fun foo\nabc", edits)).toBe("fun bar\nbaz");
  });

  test("normalizes Move Analyzer locations and workspace edits", () => {
    const range = {
      start: { line: 1, character: 2 },
      end: { line: 1, character: 5 },
    };

    expect(normalizeMoveAnalyzerLocations("/tmp/demo", {
      uri: "file:///tmp/demo/sources/main.move",
      range,
    })).toEqual([
      {
        path: "sources/main.move",
        range,
        uri: "file:///tmp/demo/sources/main.move",
      },
    ]);

    expect(normalizeMoveAnalyzerWorkspaceEdit("/tmp/demo", {
      changes: {
        "file:///tmp/demo/sources/main.move": [
          {
            range,
            newText: "renamed",
          },
        ],
      },
    })?.editsByPath["sources/main.move"][0].newText).toBe("renamed");
  });
});

describe("desktop runtime agent store", () => {
  test("removes persisted desktop agent rosters from project metadata", () => {
    const metadata = {
      ...defaultProjectMetadata(),
      agents: {
        agents: [
          {
            id: "agent-orchestrator",
            kind: "default",
            name: "Orchestrator Agent",
            description: "Old desktop audit agent.",
            status: "running",
          },
        ],
        workflows: [],
        logs: [],
        selectedAgentId: "missing",
        selectedWorkflowId: "missing",
      },
    };

    const state = loadAgentStudioStateFromProjectMetadata(metadata);
    const nextMetadata = agentStudioStateToProjectMetadata(metadata, state);

    expect(state.agents).toEqual([]);
    expect(state.workflows).toEqual([]);
    expect(nextMetadata.agents?.agents).toEqual([]);
    expect(nextMetadata.agents?.workflows).toEqual([]);
  });

  test("tracks app-server threads instead of hydrating a role roster", () => {
    const primaryOnly = ensurePrimaryAgentThreadState(
      loadAgentStudioStateFromProjectMetadata(defaultProjectMetadata()),
    );
    const state = syncAgentStudioStateWithServerThread(primaryOnly, appServerThread({
      id: "thread-worker",
      agentNickname: "audit",
      agentRole: "worker",
    }));

    expect(primaryOnly.agents.map((agent) => agent.name)).toEqual(["Main [default]"]);
    expect(state.agents.map((agent) => agent.name)).toEqual(["Main [default]", "audit [worker]"]);
    expect(state.agents.map((agent) => agent.kind)).toEqual(["server", "server"]);
    expect(state.agents[1].serverThreadId).toBe("thread-worker");
    expect("source" in state.agents[1]).toBe(false);
    expect(state.selectedAgentId).toBe("main");
  });

  test("matches TUI agent picker naming rules", () => {
    expect(formatAgentThreadName({ isPrimary: true })).toBe("Main [default]");
    expect(formatAgentThreadName({
      agentNickname: "audit",
      agentRole: "worker",
      isPrimary: false,
    })).toBe("audit [worker]");
    expect(formatAgentThreadName({ agentRole: "worker", isPrimary: false })).toBe("[worker]");
    expect(formatAgentThreadName({ isPrimary: false })).toBe("Agent");
  });
});

describe("desktop runtime app-server event mapping", () => {
  test("maps app-server text and reasoning deltas to run stream events", () => {
    expect(mapAgentServerNotificationToRunEvents({
      method: "item/agentMessage/delta",
      params: {
        threadId: "thread-1",
        turnId: "turn-1",
        itemId: "item-1",
        delta: "hello",
      },
    })).toEqual([{ type: "text-delta", text: "hello" }]);

    expect(mapAgentServerNotificationToRunEvents({
      method: "item/reasoning/summaryTextDelta",
      params: {
        threadId: "thread-1",
        turnId: "turn-1",
        itemId: "item-2",
        delta: "thinking",
      },
    })).toEqual([{ type: "reasoning-delta", text: "thinking" }]);
  });

  test("maps app-server status, finish, and error notifications", () => {
    const commandItemStarted = {
      method: "item/started",
      params: {
        threadId: "thread-1",
        turnId: "turn-1",
        item: {
          type: "commandExecution",
          id: "item-3",
          command: "cargo test",
        },
      },
    } as unknown as ServerNotification;

    expect(mapAgentServerNotificationToRunEvents(commandItemStarted)).toEqual([{
      type: "status",
      level: "trace",
      title: "Item started",
      message: "cargo test",
    }]);

    expect(mapAgentServerNotificationToRunEvents({
      method: "item/commandExecution/outputDelta",
      params: {
        threadId: "thread-1",
        turnId: "turn-1",
        itemId: "item-3",
        delta: "ok",
      },
    })).toEqual([{
      type: "status",
      level: "trace",
      title: "Command output",
      message: "ok",
    }]);

    const completed = {
      method: "turn/completed",
      params: {
        threadId: "thread-1",
        turn: {
          id: "turn-1",
          status: "completed",
        },
      },
    } as unknown as ServerNotification;

    expect(mapAgentServerNotificationToRunEvents(completed)).toEqual([{
      type: "finish",
      finishReason: "completed",
    }]);

    const error = {
      method: "error",
      params: {
        error: {
          code: -32000,
          message: "failed",
        },
      },
    } as unknown as ServerNotification;

    expect(mapAgentServerNotificationToRunEvents(error)).toEqual([{
      type: "error",
      message: "failed",
    }]);
  });

  test("maps app-server thread lifecycle notifications", () => {
    const thread = appServerThread({ id: "thread-worker", agentRole: "worker" });

    expect(mapAgentServerNotificationToRunEvents({
      method: "thread/started",
      params: { thread },
    })).toEqual([{ type: "thread-started", thread }]);

    expect(mapAgentServerNotificationToRunEvents({
      method: "thread/closed",
      params: { threadId: "thread-worker" },
    })).toEqual([{ type: "thread-closed", threadId: "thread-worker" }]);
  });
});

function appServerThread(overrides: Partial<Thread>): Thread {
  return {
    id: "thread-main",
    sessionId: "session-1",
    forkedFromId: null,
    preview: "",
    ephemeral: true,
    modelProvider: "openai",
    createdAt: 0,
    updatedAt: 0,
    status: { type: "idle" },
    path: null,
    cwd: "/tmp/demo",
    cliVersion: "test",
    source: { type: "custom", value: "test" },
    threadSource: null,
    agentNickname: null,
    agentRole: null,
    gitInfo: null,
    name: null,
    turns: [],
    ...overrides,
  } as unknown as Thread;
}
