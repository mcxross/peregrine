import {
  chmodSync,
  constants,
  copyFileSync,
  mkdirSync,
  readFileSync,
  renameSync,
  rmSync,
} from "node:fs";
import { isAbsolute, join, resolve } from "node:path";
import {
  platformBinaryName,
  resolveRustHostTriple,
  run,
  sha256,
  sidecars,
  verifySidecars,
} from "./sidecars";

const root = process.cwd();
const release = process.argv.includes("--release");
const includeTui = process.argv.includes("--include-tui");
const profile = release ? "release" : "debug";
const explicitTarget =
  argumentValue("--target") ??
  process.env.PEREGRINE_TARGET_TRIPLE ??
  process.env.CARGO_BUILD_TARGET ??
  process.env.TAURI_ENV_TARGET_TRIPLE;
const hostTriple = resolveRustHostTriple(root);
const targetTriple = explicitTarget ?? hostTriple;
if (targetTriple !== hostTriple) {
  throw new Error(
    `Sidecar packaging requires a native build so every executable can be preflighted. Host is ${hostTriple}, requested target is ${targetTriple}. Build the bundle on the target platform.`,
  );
}
const targetDirectory = resolveTargetDirectory();
const cargoOutputDirectory = explicitTarget
  ? join(targetDirectory, targetTriple, profile)
  : join(targetDirectory, profile);
const destinationDirectory = join(root, "src-tauri", "binaries");

validateTauriExternalBins();

const cargoArgs = ["build", "--locked"];

// Fix movy-replay build by pointing it to the local sui checkout
if (!process.env.MOVY_SUI_ROOT) {
  try {
    const metadata = JSON.parse(
      require("node:child_process").execSync("cargo metadata --format-version 1 --locked", {
        encoding: "utf8",
        maxBuffer: 1024 * 1024 * 100, // 100MB buffer for large metadata
      }),
    );
    const suiPackage = metadata.packages.find((p: any) => p.name === "sui" && p.source?.includes("git"));
    if (suiPackage && suiPackage.manifest_path) {
      process.env.MOVY_SUI_ROOT = require("node:path").resolve(
        require("node:path").dirname(suiPackage.manifest_path),
        "..",
        "..",
      );
      console.log(`Resolved MOVY_SUI_ROOT to ${process.env.MOVY_SUI_ROOT}`);
    }
  } catch (e) {
    console.warn("Failed to automatically resolve MOVY_SUI_ROOT from cargo metadata");
  }
}

for (const sidecar of sidecars) {
  cargoArgs.push("-p", sidecar.packageName);
}
if (includeTui) {
  cargoArgs.push("-p", "peregrine-tui");
}
if (release) {
  cargoArgs.push("--release");
}
if (explicitTarget) {
  cargoArgs.push("--target", targetTriple);
}
run("cargo", cargoArgs, root);

mkdirSync(destinationDirectory, { recursive: true });
const prepared = new Map<string, string>();
for (const sidecar of sidecars) {
  const fileName = platformBinaryName(sidecar.binaryName);
  const source = join(cargoOutputDirectory, fileName);
  const destination = join(
    destinationDirectory,
    platformBinaryName(`${sidecar.binaryName}-${targetTriple}`),
  );
  const temporaryDestination = `${destination}.tmp`;

  rmSync(temporaryDestination, { force: true });
  copyFileSync(source, temporaryDestination, constants.COPYFILE_FICLONE);
  if (process.platform !== "win32") {
    chmodSync(temporaryDestination, 0o755);
  }
  rmSync(destination, { force: true });
  renameSync(temporaryDestination, destination);
  if ((await sha256(source)) !== (await sha256(destination))) {
    throw new Error(`Sidecar copy verification failed for ${sidecar.binaryName}`);
  }
  prepared.set(sidecar.binaryName, destination);
  console.log(`Prepared ${destination}`);
}

verifySidecars(prepared);
console.log(`Verified ${sidecars.length} isolated Peregrine sidecars.`);

function resolveTargetDirectory() {
  const configured = process.env.CARGO_TARGET_DIR;
  if (!configured) {
    return join(root, "target");
  }
  return isAbsolute(configured) ? configured : resolve(root, configured);
}

function argumentValue(name: string) {
  const equalsArgument = process.argv.find((argument) =>
    argument.startsWith(`${name}=`),
  );
  if (equalsArgument) {
    return equalsArgument.slice(name.length + 1);
  }
  const index = process.argv.indexOf(name);
  if (index < 0) {
    return undefined;
  }
  const value = process.argv[index + 1];
  if (!value || value.startsWith("--")) {
    throw new Error(`${name} requires a target triple`);
  }
  return value;
}

function validateTauriExternalBins() {
  const config = JSON.parse(
    readFileSync(join(root, "src-tauri", "tauri.conf.json"), "utf8"),
  );
  const configured = config.bundle?.externalBin?.map((entry: string) =>
    entry.replace(/^binaries\//, ""),
  );
  const expected = sidecars.map((sidecar) => sidecar.binaryName);
  if (JSON.stringify(configured) !== JSON.stringify(expected)) {
    throw new Error(
      `Tauri externalBin does not match the sidecar manifest:\nexpected ${JSON.stringify(expected)}\nactual   ${JSON.stringify(configured)}`,
    );
  }
}
