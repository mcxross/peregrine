import { spawnSync } from "node:child_process";
import {
  X_OK,
  accessSync,
  createReadStream,
  existsSync,
  statSync,
} from "node:fs";
import { createHash } from "node:crypto";

export const sidecars = [
  {
    binaryName: "peregrine-helper",
    packageName: "peregrine-helper",
    tools: undefined,
    smokeTool: undefined,
  },
  {
    binaryName: "peregrine-sui-mcp-server",
    packageName: "peregrine-sui-mcp-server",
    tools: [
      "package_resolve",
      "modules",
      "signatures",
      "import_package",
      "create_package",
      "static_rule_catalog",
      "static_analyze_package",
      "scanner_report",
      "test_scanner_report",
      "package_insights",
      "graphs",
      "function_state_graph",
      "bytecode_view",
      "bytecode_decompile",
      "command",
      "movy_fuzz",
      "formal_verify",
      "analyze",
    ],
    smokeTool: undefined,
  },
  {
    binaryName: "peregrine-sui-move-analyzer-mcp-server",
    packageName: "peregrine-sui-move-analyzer-mcp-server",
    tools: [
      "status",
      "diagnostics",
      "completion",
      "hover",
      "definition",
      "references",
      "rename",
    ],
    smokeTool: "status",
  },
] as const;

export function platformBinaryName(binaryName: string) {
  return process.platform === "win32" ? `${binaryName}.exe` : binaryName;
}

export function resolveRustHostTriple(cwd: string) {
  const hostTuple = spawnSync("rustc", ["--print", "host-tuple"], {
    cwd,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  if (hostTuple.status === 0 && hostTuple.stdout.trim()) {
    return hostTuple.stdout.trim();
  }

  const version = spawnSync("rustc", ["-Vv"], {
    cwd,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  const triple = version.stdout
    .split(/\r?\n/)
    .find((line) => line.startsWith("host:"))
    ?.replace("host:", "")
    .trim();
  if (!triple) {
    throw new Error(
      version.stderr.trim() || "Could not resolve the Rust host target triple.",
    );
  }
  return triple;
}

export function verifySidecars(paths: ReadonlyMap<string, string>) {
  for (const sidecar of sidecars) {
    const executable = requiredPath(paths, sidecar.binaryName);
    verifyExecutable(executable);
  }

  const helper = requiredPath(paths, "peregrine-helper");
  verifyHelper(helper);

  for (const sidecar of sidecars) {
    if (sidecar.tools) {
      verifyMcpServer(
        requiredPath(paths, sidecar.binaryName),
        helper,
        sidecar.tools,
        sidecar.smokeTool,
      );
    }
  }
}

export function run(command: string, args: string[], cwd: string) {
  const result = spawnSync(command, args, {
    cwd,
    stdio: "inherit",
    env: process.env,
  });

  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(
      `${command} ${args.join(" ")} failed with status ${result.status}`,
    );
  }
}

export async function sha256(path: string) {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(path)) {
    hash.update(chunk);
  }
  return hash.digest("hex");
}

function requiredPath(paths: ReadonlyMap<string, string>, binaryName: string) {
  const path = paths.get(binaryName);
  if (!path) {
    throw new Error(`No prepared path was provided for ${binaryName}`);
  }
  return path;
}

function verifyExecutable(executable: string) {
  if (!existsSync(executable)) {
    throw new Error(`Expected sidecar binary at ${executable}`);
  }
  const metadata = statSync(executable);
  if (!metadata.isFile() || metadata.size === 0) {
    throw new Error(`Sidecar is not a non-empty file: ${executable}`);
  }
  if (process.platform !== "win32") {
    accessSync(executable, X_OK);
  }
}

function verifyHelper(executable: string) {
  const result = spawnSync(executable, ["--peregrine-helper-json"], {
    input: JSON.stringify({ kind: "ping" }),
    encoding: "utf8",
    timeout: 20_000,
    maxBuffer: 1024 * 1024,
  });
  assertSuccessfulProcess(result, "Peregrine helper preflight");

  const response = parseJsonLine(result.stdout, "Peregrine helper preflight");
  if (response.ok !== true || response.status !== 0) {
    throw new Error("Peregrine helper preflight returned an invalid response");
  }
}

function verifyMcpServer(
  executable: string,
  helper: string,
  expectedTools: readonly string[],
  smokeTool: string | undefined,
) {
  const requests = [
    {
      jsonrpc: "2.0",
      id: 1,
      method: "initialize",
      params: {
        protocolVersion: "2025-06-18",
        capabilities: {},
        clientInfo: {
          name: "peregrine-sidecar-preflight",
          version: "1",
        },
      },
    },
    {
      jsonrpc: "2.0",
      method: "notifications/initialized",
      params: {},
    },
    {
      jsonrpc: "2.0",
      id: 2,
      method: "tools/list",
      params: {},
    },
  ];
  if (smokeTool) {
    requests.push({
      jsonrpc: "2.0",
      id: 3,
      method: "tools/call",
      params: {
        name: smokeTool,
        arguments: {},
      },
    });
  }
  const result = spawnSync(executable, [], {
    cwd: process.cwd(),
    env: {
      ...process.env,
      PEREGRINE_HELPER: helper,
    },
    input: `${requests.map(JSON.stringify).join("\n")}\n`,
    encoding: "utf8",
    timeout: 20_000,
    maxBuffer: 1024 * 1024,
  });
  assertSuccessfulProcess(result, `${executable} MCP preflight`);

  const responses = result.stdout
    .split(/\r?\n/)
    .filter(Boolean)
    .map((line) => JSON.parse(line));
  const initialize = responses.find((response) => response.id === 1);
  const tools = responses.find((response) => response.id === 2);
  if (!initialize?.result?.serverInfo) {
    throw new Error(`${executable} returned an invalid initialize response`);
  }

  const actualTools = tools?.result?.tools?.map(
    (tool: { name: string }) => tool.name,
  );
  if (JSON.stringify(actualTools) !== JSON.stringify(expectedTools)) {
    throw new Error(
      `${executable} tool inventory mismatch:\nexpected ${JSON.stringify(expectedTools)}\nactual   ${JSON.stringify(actualTools)}`,
    );
  }

  if (smokeTool) {
    const smokeResult = responses.find((response) => response.id === 3);
    if (!smokeResult?.result || smokeResult.error || smokeResult.result.isError) {
      throw new Error(
        `${executable} failed the ${smokeTool} smoke test: ${JSON.stringify(smokeResult)}`,
      );
    }
  }
}

function assertSuccessfulProcess(
  result: ReturnType<typeof spawnSync>,
  operation: string,
) {
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(
      result.stderr?.toString().trim() ||
        `${operation} failed with status ${result.status}`,
    );
  }
}

function parseJsonLine(raw: string, operation: string) {
  try {
    return JSON.parse(raw.trim());
  } catch (error) {
    throw new Error(`${operation} returned invalid JSON: ${error}`);
  }
}
