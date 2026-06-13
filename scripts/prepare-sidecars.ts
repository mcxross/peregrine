import { spawnSync } from "node:child_process";
import {
  constants,
  copyFileSync,
  existsSync,
  mkdirSync,
  rmSync,
} from "node:fs";
import { join } from "node:path";

const root = process.cwd();
const release = process.argv.includes("--release");
const profile = release ? "release" : "debug";
const extension = process.platform === "win32" ? ".exe" : "";
const targetTriple = resolveTargetTriple();
const destinationDirectory = join(root, "src-tauri", "binaries");
const sidecars = ["peregrine-helper", "peregrine-sui-mcp-server"];

for (const sidecar of sidecars) {
  run("cargo", ["build", "-p", sidecar, ...(release ? ["--release"] : [])]);
}

mkdirSync(destinationDirectory, { recursive: true });
for (const sidecar of sidecars) {
  const source = join(root, "target", profile, `${sidecar}${extension}`);
  const destination = join(
    destinationDirectory,
    `${sidecar}-${targetTriple}${extension}`,
  );

  if (!existsSync(source)) {
    throw new Error(`Expected sidecar binary at ${source}`);
  }

  rmSync(destination, { force: true });
  copyFileSync(source, destination, constants.COPYFILE_FICLONE);
  console.log(`Prepared ${destination}`);

  if (sidecar === "peregrine-sui-mcp-server") {
    verifyMcpServer(destination);
  }
}

function resolveTargetTriple() {
  const hostTuple = spawnSync("rustc", ["--print", "host-tuple"], {
    cwd: root,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });

  if (hostTuple.status === 0) {
    const triple = hostTuple.stdout.trim();

    if (triple) {
      return triple;
    }
  }

  const version = spawnSync("rustc", ["-Vv"], {
    cwd: root,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });

  if (version.status !== 0) {
    throw new Error(version.stderr.trim() || "Could not resolve Rust host target triple.");
  }

  const hostLine = version.stdout
    .split(/\r?\n/)
    .find((line) => line.startsWith("host:"));
  const triple = hostLine?.replace("host:", "").trim();

  if (!triple) {
    throw new Error("Could not parse Rust host target triple.");
  }

  return triple;
}

function run(command: string, args: string[]) {
  const result = spawnSync(command, args, {
    cwd: root,
    stdio: "inherit",
  });

  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed with status ${result.status}`);
  }
}

function verifyMcpServer(executable: string) {
  const request = JSON.stringify({
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
  });
  const result = spawnSync(executable, [], {
    cwd: root,
    input: `${request}\n`,
    encoding: "utf8",
    timeout: 20_000,
    maxBuffer: 1024 * 1024,
  });

  if (result.status !== 0) {
    throw new Error(
      result.error?.message ||
        result.stderr.trim() ||
        `MCP sidecar preflight failed with status ${result.status}`,
    );
  }

  const response = JSON.parse(result.stdout.trim());
  if (response.id !== 1 || !response.result?.serverInfo) {
    throw new Error("MCP sidecar preflight returned an invalid initialize response");
  }
}
