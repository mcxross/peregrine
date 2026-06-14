import { existsSync, readdirSync, statSync } from "node:fs";
import { basename, join, resolve } from "node:path";
import {
  platformBinaryName,
  resolveRustHostTriple,
  run,
  sidecars,
  verifySidecars,
} from "./sidecars";

const root = process.cwd();
if (process.platform === "darwin") {
  const appBundle = resolveMacAppBundle();
  const prepared = sidecarsInDirectory(join(appBundle, "Contents", "MacOS"));
  verifySidecars(prepared);
  for (const executable of prepared.values()) {
    run("codesign", ["--verify", "--strict", executable], root);
  }
  run("codesign", ["--verify", "--deep", "--strict", appBundle], root);
  console.log(`Verified packaged desktop sidecars in ${basename(appBundle)}.`);
} else if (process.argv[2]) {
  const unpackedBundle = resolve(root, process.argv[2]);
  verifySidecars(sidecarsInDirectory(unpackedBundle));
  console.log(`Verified packaged desktop sidecars in ${unpackedBundle}.`);
} else {
  const targetTriple = resolveRustHostTriple(root);
  const sidecarInputs = new Map(
    sidecars.map((sidecar) => [
      sidecar.binaryName,
      join(
        root,
        "src-tauri",
        "binaries",
        platformBinaryName(`${sidecar.binaryName}-${targetTriple}`),
      ),
    ]),
  );
  verifySidecars(sidecarInputs);
  console.log(
    `Verified the ${targetTriple} sidecar inputs consumed by the desktop bundle.`,
  );
}

function sidecarsInDirectory(directory: string) {
  return new Map(
    sidecars.map((sidecar) => [
      sidecar.binaryName,
      join(directory, platformBinaryName(sidecar.binaryName)),
    ]),
  );
}

function resolveMacAppBundle() {
  if (process.argv[2]) {
    const configured = resolve(root, process.argv[2]);
    if (!existsSync(configured)) {
      throw new Error(`Desktop app bundle was not found at ${configured}`);
    }
    return configured;
  }

  const targetDirectory = resolve(root, process.env.CARGO_TARGET_DIR ?? "target");
  const candidates = [
    join(targetDirectory, "release", "bundle", "macos", "Peregrine.app"),
    ...targetSubdirectories(targetDirectory).map((directory) =>
      join(directory, "release", "bundle", "macos", "Peregrine.app"),
    ),
  ].filter(existsSync);

  if (candidates.length === 0) {
    throw new Error(
      `Desktop app bundle was not found under ${targetDirectory}. Pass its path explicitly.`,
    );
  }
  return candidates.toSorted(
    (left, right) => statSync(right).mtimeMs - statSync(left).mtimeMs,
  )[0];
}

function targetSubdirectories(targetDirectory: string) {
  if (!existsSync(targetDirectory)) {
    return [];
  }
  return readdirSync(targetDirectory, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => join(targetDirectory, entry.name));
}
