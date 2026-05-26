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
  defaultAgents,
} from "../src/agents/default-agents";
import {
  loadAgentStudioStateFromProjectMetadata,
} from "../src/agents/agent-workflow-store";

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
  test("normalizes persisted agent metadata against current defaults", () => {
    const firstDefault = defaultAgents[0];
    const metadata = {
      ...defaultProjectMetadata(),
      agents: {
        agents: [
          {
            ...firstDefault,
            tools: ["custom.extra"],
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
    const normalizedDefault = state.agents.find((agent) => agent.id === firstDefault.id);

    expect(normalizedDefault?.tools).toContain(firstDefault.tools[0]);
    expect(normalizedDefault?.tools).toContain("custom.extra");
    expect(state.selectedAgentId).toBe(defaultAgents[0].id);
  });
});
