#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { mkdir } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

const repoRoot = path.resolve(import.meta.dirname, "..");
const distRoot = path.join(repoRoot, "dist", "windows");
const nsisScript = path.join(repoRoot, "scripts", "windows", "foco.nsi");
const targetRoot = path.resolve(repoRoot, process.env.CARGO_TARGET_DIR ?? "target");
const defaultAppExe = path.join(targetRoot, "release", "foco.exe");
const defaultResources = path.join(targetRoot, "release", "resources");

try {
  const options = parseArgs(process.argv.slice(2));
  const version = options.version ?? cargoPackageVersion();
  const outFile = path.resolve(options.outFile ?? path.join(distRoot, "Foco-windows-x64-setup.exe"));
  const appExe = path.resolve(options.appExe ?? defaultAppExe);
  const appResources = path.resolve(options.appResources ?? defaultResources);
  const makensis = resolveMakensis(options.makensis, options.dryRun);

  if (!options.dryRun) {
    assertWindowsHost();
    assertMakensis(makensis);
    runNpm(["run", "build:release", "--", "--bundle-sidecars"]);
    assertFile(appExe, "release executable");
    assertDirectory(appResources, "release resources directory");
    await mkdir(path.dirname(outFile), { recursive: true });
    runMakensis(makensis, { appExe, appResources, version, outFile });
    console.log(`[windows] packaged ${outFile}`);
  } else {
    console.log(JSON.stringify({ makensis, appExe, appResources, version, outFile, nsisScript }, null, 2));
  }
} catch (error) {
  console.error(`[windows] ${errorMessage(error)}`);
  process.exitCode = 1;
}

function parseArgs(args) {
  const options = { dryRun: false };

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--dry-run") {
      options.dryRun = true;
    } else if (arg === "--version") {
      options.version = readValue(args, ++index, arg);
    } else if (arg === "--out-file") {
      options.outFile = readValue(args, ++index, arg);
    } else if (arg === "--app-exe") {
      options.appExe = readValue(args, ++index, arg);
    } else if (arg === "--app-resources") {
      options.appResources = readValue(args, ++index, arg);
    } else if (arg === "--makensis") {
      options.makensis = readValue(args, ++index, arg);
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }

  return options;
}

function readValue(args, index, flag) {
  const value = args[index];
  if (!value || value.startsWith("--")) {
    throw new Error(`${flag} requires a value`);
  }
  return value;
}

function assertWindowsHost() {
  if (process.platform !== "win32") {
    throw new Error("Windows installer packaging must run on Windows.");
  }
}

function resolveMakensis(explicitMakensis, dryRun) {
  if (explicitMakensis) {
    return explicitMakensis;
  }
  if (dryRun) {
    return process.platform === "win32" ? "makensis.exe" : "makensis";
  }
  return process.platform === "win32" ? "makensis.exe" : "makensis";
}

function assertMakensis(makensis) {
  const result = spawnSync(makensis, ["/VERSION"], {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });

  if (result.error?.code === "ENOENT") {
    throw new Error("makensis was not found. Install NSIS and ensure makensis.exe is on PATH.");
  }
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`makensis /VERSION exited with code ${result.status}: ${result.stderr.trim()}`);
  }
}

function runMakensis(makensis, { appExe, appResources, version, outFile }) {
  const productVersion = nsisProductVersion(version);
  run(makensis, [
    `/DAPP_EXE=${appExe}`,
    `/DAPP_RESOURCES=${appResources}`,
    `/DVERSION=${version}`,
    `/DPRODUCT_VERSION=${productVersion}`,
    `/DOUT_FILE=${outFile}`,
    nsisScript,
  ]);
}

function nsisProductVersion(version) {
  const numeric = version
    .replace(/^v/i, "")
    .split(/[.-]/)
    .filter((part) => /^\d+$/.test(part))
    .slice(0, 4);
  while (numeric.length < 4) {
    numeric.push("0");
  }
  return numeric.join(".");
}

function cargoPackageVersion() {
  const result = spawnSync("cargo", ["metadata", "--no-deps", "--format-version", "1"], {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });

  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`cargo metadata exited with code ${result.status}: ${result.stderr.trim()}`);
  }

  const metadata = JSON.parse(result.stdout);
  const appPackage = metadata.packages.find((pkg) => pkg.name === "foco-app");
  if (!appPackage?.version) {
    throw new Error("cargo metadata did not include foco-app version");
  }
  return appPackage.version;
}

function assertFile(filePath, label) {
  if (!existsSync(filePath)) {
    throw new Error(`${label} was not created: ${filePath}`);
  }
}

function assertDirectory(directoryPath, label) {
  if (!existsSync(directoryPath)) {
    throw new Error(`${label} was not created: ${directoryPath}`);
  }
}

function runNpm(args) {
  run("cmd.exe", ["/d", "/s", "/c", "npm.cmd", ...args]);
}

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    env: process.env,
    stdio: "inherit",
    windowsHide: true,
  });

  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} exited with code ${result.status}`);
  }
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}
