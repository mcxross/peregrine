import { spawnSync } from "node:child_process";
import { existsSync, readdirSync, statSync, readFileSync } from "node:fs";
import { join, relative } from "node:path";
import { gzipSync } from "node:zlib";
import { performance } from "node:perf_hooks";

type AssetMeasurement = {
  file: string;
  gzipBytes: number;
  rawBytes: number;
};

type CliMeasurement = {
  args: string[];
  command: string;
  medianMs: number;
  samplesMs: number[];
  status: number | null;
};

type BinaryMeasurement = {
  bytes: number;
  file: string;
};

type PerformanceReport = {
  binaries: BinaryMeasurement[];
  cli: CliMeasurement[];
  frontend: {
    assets: AssetMeasurement[];
    totalGzipBytes: number;
    totalRawBytes: number;
  };
};

const root = process.cwd();
const args = new Set(process.argv.slice(2));
const outputJson = args.has("--json");
const cliSamples = Number(process.env.PEREGRINE_MEASURE_CLI_SAMPLES ?? 7);
const cliWarmups = Number(process.env.PEREGRINE_MEASURE_CLI_WARMUPS ?? 1);

function measureAssets(): PerformanceReport["frontend"] {
  const assetsRoot = join(root, "dist", "assets");

  if (!existsSync(assetsRoot)) {
    return {
      assets: [],
      totalGzipBytes: 0,
      totalRawBytes: 0,
    };
  }

  const assets = readdirSync(assetsRoot)
    .filter((file) => file.endsWith(".js") || file.endsWith(".css"))
    .map((file) => {
      const path = join(assetsRoot, file);
      const contents = readFileSync(path);

      return {
        file: relative(root, path),
        gzipBytes: gzipSync(contents).byteLength,
        rawBytes: contents.byteLength,
      };
    })
    .sort((left, right) => right.rawBytes - left.rawBytes);

  return {
    assets,
    totalGzipBytes: assets.reduce((total, asset) => total + asset.gzipBytes, 0),
    totalRawBytes: assets.reduce((total, asset) => total + asset.rawBytes, 0),
  };
}

function measureCli(): CliMeasurement[] {
  const binary = resolveExistingPath([
    join(root, "target", "release", "peregrine-cli"),
    join(root, "target", "debug", "peregrine-cli"),
  ]);

  if (!binary) {
    return [];
  }

  const commands = [
    ["--help"],
    ["--version"],
  ];

  return commands.map((commandArgs) => measureCommand(binary, commandArgs));
}

function measureCommand(command: string, commandArgs: string[]): CliMeasurement {
  for (let index = 0; index < cliWarmups; index += 1) {
    spawnSync(command, commandArgs, {
      cwd: root,
      encoding: "utf8",
      stdio: "pipe",
    });
  }

  const samplesMs: number[] = [];
  let status: number | null = null;

  for (let index = 0; index < cliSamples; index += 1) {
    const startedAt = performance.now();
    const result = spawnSync(command, commandArgs, {
      cwd: root,
      encoding: "utf8",
      stdio: "pipe",
    });

    samplesMs.push(performance.now() - startedAt);
    status = result.status;
  }

  return {
    args: commandArgs,
    command: relative(root, command),
    medianMs: median(samplesMs),
    samplesMs,
    status,
  };
}

function measureBinaries(): BinaryMeasurement[] {
  return [
    join(root, "target", "release", "peregrine"),
    join(root, "target", "release", "peregrine-cli"),
    join(root, "target", "release", "peregrine-helper"),
    join(root, "target", "debug", "peregrine"),
    join(root, "target", "debug", "peregrine-cli"),
    join(root, "target", "debug", "peregrine-helper"),
  ]
    .filter((file) => existsSync(file))
    .map((file) => ({
      bytes: statSync(file).size,
      file: relative(root, file),
    }))
    .sort((left, right) => left.file.localeCompare(right.file));
}

function resolveExistingPath(candidates: string[]) {
  return candidates.find((candidate) => existsSync(candidate)) ?? null;
}

function median(values: number[]) {
  const sorted = [...values].sort((left, right) => left - right);
  const middle = Math.floor(sorted.length / 2);

  if (sorted.length % 2 === 1) {
    return sorted[middle] ?? 0;
  }

  return ((sorted[middle - 1] ?? 0) + (sorted[middle] ?? 0)) / 2;
}

function formatBytes(bytes: number) {
  if (bytes < 1024) {
    return `${bytes} B`;
  }

  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KiB`;
  }

  return `${(bytes / 1024 / 1024).toFixed(2)} MiB`;
}

function printReport(report: PerformanceReport) {
  console.log("Frontend assets");
  if (report.frontend.assets.length === 0) {
    console.log("  dist/assets not found; run bun run build first.");
  } else {
    console.log(
      `  total: ${formatBytes(report.frontend.totalRawBytes)} raw / ${formatBytes(
        report.frontend.totalGzipBytes,
      )} gzip`,
    );
    for (const asset of report.frontend.assets.slice(0, 8)) {
      console.log(
        `  ${asset.file}: ${formatBytes(asset.rawBytes)} raw / ${formatBytes(asset.gzipBytes)} gzip`,
      );
    }
  }

  console.log("");
  console.log("CLI startup");
  if (report.cli.length === 0) {
    console.log("  target/{debug,release}/peregrine-cli not found; run cargo build -p peregrine-cli first.");
  } else {
    for (const measurement of report.cli) {
      console.log(
        `  ${measurement.command} ${measurement.args.join(" ")}: ${measurement.medianMs.toFixed(
          1,
        )} ms median (${measurement.samplesMs.map((sample) => sample.toFixed(1)).join(", ")} ms)`,
      );
    }
  }

  console.log("");
  console.log("Binaries");
  if (report.binaries.length === 0) {
    console.log("  no target binaries found.");
  } else {
    for (const binary of report.binaries) {
      console.log(`  ${binary.file}: ${formatBytes(binary.bytes)}`);
    }
  }
}

const report: PerformanceReport = {
  binaries: measureBinaries(),
  cli: measureCli(),
  frontend: measureAssets(),
};

if (outputJson) {
  console.log(JSON.stringify(report, null, 2));
} else {
  printReport(report);
}
