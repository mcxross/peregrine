import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { join } from "node:path";

const root = process.cwd();
const release = process.argv.includes("--release");
const profile = release ? "release" : "debug";
const extension = process.platform === "win32" ? ".exe" : "";
const targetTriple = resolveTargetTriple();
const source = join(root, "target", profile, `peregrine-helper${extension}`);
const destinationDirectory = join(root, "src-tauri", "binaries");
const destination = join(destinationDirectory, `peregrine-helper-${targetTriple}${extension}`);

run("cargo", ["build", "-p", "peregrine-helper", ...(release ? ["--release"] : [])]);

if (!existsSync(source)) {
  throw new Error(`Expected helper binary at ${source}`);
}

mkdirSync(destinationDirectory, { recursive: true });
copyFileSync(source, destination);
console.log(`Prepared ${destination}`);

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
