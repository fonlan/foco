#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { createReadStream, existsSync } from "node:fs";
import { chmod, copyFile, cp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

const repoRoot = path.resolve(import.meta.dirname, "..");
const defaultSidecarsRoot = path.join(repoRoot, "sidecars");

try {
  const { command, options } = parseArgs(process.argv.slice(2));

  if (command === "manifest") {
    await writeManifestEntry(options);
  } else if (command === "verify") {
    await verifySidecars(options.root ?? defaultSidecarsRoot);
  } else if (command === "copy") {
    await copySidecars(options);
  } else {
    throw new Error(`unknown sidecars command: ${command ?? "<missing>"}`);
  }
} catch (error) {
  console.error(`[sidecars] ${errorMessage(error)}`);
  process.exitCode = 1;
}

async function writeManifestEntry(options) {
  const target = requiredOption(options, "target", "manifest");
  const binary = path.resolve(requiredOption(options, "binary", "manifest"));
  const sidecarsRoot = path.resolve(options.out ?? defaultSidecarsRoot);
  const version = options.version ?? cargoPackageVersion();

  if (!existsSync(binary)) {
    throw new Error(`sidecar binary does not exist: ${binary}`);
  }

  const relativePath = path.posix.join(target, "foco");
  const destination = path.join(sidecarsRoot, ...relativePath.split("/"));
  await mkdir(path.dirname(destination), { recursive: true });
  await copyFile(binary, destination);
  await chmod(destination, 0o755).catch((error) => {
    if (process.platform !== "win32") {
      throw error;
    }
  });

  const manifestPath = path.join(sidecarsRoot, "manifest.json");
  const manifest = existsSync(manifestPath)
    ? JSON.parse(await readFile(manifestPath, "utf8"))
    : { version, sidecars: [] };

  if (manifest.version !== version) {
    throw new Error(
      `existing sidecar manifest version ${JSON.stringify(manifest.version)} does not match ${JSON.stringify(version)}`,
    );
  }

  manifest.sidecars = manifest.sidecars.filter((entry) => entry.target !== target);
  manifest.sidecars.push({
    target,
    path: relativePath,
    sha256: await sha256File(destination),
  });
  manifest.sidecars.sort((left, right) => left.target.localeCompare(right.target));

  await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
  await verifySidecars(sidecarsRoot);
  console.log(`[sidecars] wrote ${manifestPath}`);
}

async function copySidecars(options) {
  const source = path.resolve(options.source ?? defaultSidecarsRoot);
  const destination = path.resolve(requiredOption(options, "dest", "copy"));

  await verifySidecars(source);
  await rm(destination, { force: true, recursive: true });
  await mkdir(path.dirname(destination), { recursive: true });
  await cp(source, destination, { recursive: true });
  await verifySidecars(destination);
  console.log(`[sidecars] copied ${source} -> ${destination}`);
}

async function verifySidecars(sidecarsRoot) {
  const manifestPath = path.join(sidecarsRoot, "manifest.json");
  if (!existsSync(manifestPath)) {
    throw new Error(`missing sidecars/manifest.json at ${manifestPath}. ${sidecarFixHint()}`);
  }

  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  if (!manifest.version || typeof manifest.version !== "string") {
    throw new Error("sidecars/manifest.json must include a string version");
  }
  if (!Array.isArray(manifest.sidecars) || manifest.sidecars.length === 0) {
    throw new Error("sidecars/manifest.json must include at least one sidecar entry");
  }

  const seenTargets = new Set();
  for (const entry of manifest.sidecars) {
    assertManifestEntry(entry, seenTargets);
    const binaryPath = path.join(sidecarsRoot, ...entry.path.split("/"));
    if (!existsSync(binaryPath)) {
      throw new Error(`manifest entry ${entry.target} points to missing file: ${entry.path}`);
    }

    const actualSha256 = await sha256File(binaryPath);
    if (actualSha256 !== entry.sha256) {
      throw new Error(
        `sha256 mismatch for ${entry.path}: manifest has ${entry.sha256}, actual is ${actualSha256}`,
      );
    }
  }
}

function assertManifestEntry(entry, seenTargets) {
  if (!entry || typeof entry !== "object") {
    throw new Error("sidecar manifest entries must be objects");
  }
  for (const field of ["target", "path", "sha256"]) {
    if (typeof entry[field] !== "string" || entry[field].length === 0) {
      throw new Error(`sidecar manifest entry must include a string ${field}`);
    }
  }
  if (seenTargets.has(entry.target)) {
    throw new Error(`duplicate sidecar target in manifest: ${entry.target}`);
  }
  seenTargets.add(entry.target);
  if (path.posix.isAbsolute(entry.path) || entry.path.includes("..") || entry.path.includes("\\")) {
    throw new Error(`sidecar manifest path must be a relative POSIX path: ${entry.path}`);
  }
  if (!/^[a-f0-9]{64}$/.test(entry.sha256)) {
    throw new Error(`sidecar sha256 must be 64 lowercase hex characters for ${entry.target}`);
  }
}

function parseArgs(args) {
  const [command, ...rest] = args;
  const options = {};

  for (let index = 0; index < rest.length; index += 1) {
    const arg = rest[index];
    if (!arg.startsWith("--")) {
      throw new Error(`unexpected argument: ${arg}`);
    }

    const key = arg.slice(2);
    const value = rest[index + 1];
    if (!value || value.startsWith("--")) {
      throw new Error(`missing value for ${arg}`);
    }
    options[key] = value;
    index += 1;
  }

  return { command, options };
}

function requiredOption(options, key, command) {
  const value = options[key];
  if (!value) {
    throw new Error(`sidecars ${command} requires --${key}`);
  }
  return value;
}

async function sha256File(filePath) {
  const hash = createHash("sha256");
  await new Promise((resolve, reject) => {
    createReadStream(filePath)
      .on("data", (chunk) => hash.update(chunk))
      .once("error", reject)
      .once("end", resolve);
  });
  return hash.digest("hex");
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

function sidecarFixHint() {
  return "Build or download Linux sidecars first; CI runs the linux-sidecars job, local release packaging can reuse its sidecars artifact.";
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}
