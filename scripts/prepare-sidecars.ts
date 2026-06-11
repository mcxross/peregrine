import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { join } from "node:path";

const root = process.cwd();
const release = process.argv.includes("--release");
const profile = release ? "release" : "debug";
const extension = process.platform === "win32" ? ".exe" : "";
const targetTriple = resolveTargetTriple();
const destinationDirectory = join(root, "src-tauri", "binaries");
const sidecars = ["peregrine-helper", "peregrine-mcp-server"];

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

  copyFileSync(source, destination);
  console.log(`Prepared ${destination}`);
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
