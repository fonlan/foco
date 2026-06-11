import { spawn } from "node:child_process";
import { readdir, stat } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import { cleanDevEnv, formatDevEnvOverrides, parseBackendDevArgs } from "./dev-args.mjs";

const repoRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);
const watchEntries = [
  "Cargo.lock",
  "Cargo.toml",
  "agent",
  "app",
  "graph",
  "mcp",
  "providers",
  "store",
  "tools",
];
const watchedExtensions = new Set([".lock", ".rs", ".toml"]);
const pollIntervalMs = 800;
const cargoCommand = process.platform === "win32" ? "cargo.exe" : "cargo";
const nodeCommand = process.execPath;
const tscScript = path.join(repoRoot, "node_modules", "typescript", "lib", "tsc.js");
const viteScript = path.join(repoRoot, "node_modules", "vite", "bin", "vite.js");
const webRoot = path.join(repoRoot, "web");

let backendProcess = null;
let currentSignature = "";
let isRestarting = false;
let isStopping = false;
let devOptions;

try {
  devOptions = parseBackendDevArgs(process.argv.slice(2), process.env);
} catch (error) {
  console.error(`[backend] ${errorMessage(error)}`);
  console.error(
    "[backend] usage: npm run backend -- <port> <config-dir>",
  );
  process.exit(1);
}
const devEnv = cleanDevEnv(process.env, devOptions.env);

await buildFrontend();
currentSignature = await sourceSignature();
startBackend();

const pollTimer = setInterval(async () => {
  if (isRestarting || isStopping) {
    return;
  }

  try {
    const nextSignature = await sourceSignature();

    if (nextSignature === currentSignature) {
      return;
    }

    currentSignature = nextSignature;
    await restartBackend();
  } catch (error) {
    console.error(`[backend] failed to scan source changes: ${errorMessage(error)}`);
  }
}, pollIntervalMs);

process.on("SIGINT", () => {
  void shutdown();
});
process.on("SIGTERM", () => {
  void shutdown();
});

function startBackend() {
  console.log(
    `[backend] starting cargo run -p foco-app${formatDevEnvOverrides(devOptions.env)}`,
  );
  backendProcess = spawn(cargoCommand, ["run", "-p", "foco-app"], {
    cwd: repoRoot,
    detached: process.platform !== "win32",
    env: devEnv,
    stdio: "inherit",
  });
  backendProcess.once("exit", (code, signal) => {
    if (!isRestarting && !isStopping) {
      console.log(`[backend] exited with ${signal ?? code ?? "unknown status"}`);
    }
  });
}

async function buildFrontend() {
  console.log("[backend] building web assets");
  await runCommand(nodeCommand, [tscScript, "-b"], webRoot);
  await runCommand(nodeCommand, [viteScript, "build"], webRoot);
}

async function runCommand(command, args, cwd) {
  await new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd,
      env: devEnv,
      stdio: "inherit",
    });

    child.once("error", reject);
    child.once("exit", (code, signal) => {
      if (code === 0) {
        resolve();
        return;
      }

      reject(
        new Error(
          `${command} ${args.join(" ")} exited with ${signal ?? code ?? "unknown status"}`,
        ),
      );
    });
  }).catch((error) => {
    console.error(`[backend] ${errorMessage(error)}`);
    process.exit(1);
  });
}

async function restartBackend() {
  isRestarting = true;
  console.log("[backend] source changed; restarting");

  try {
    await stopBackend();
    startBackend();
  } finally {
    isRestarting = false;
  }
}

async function shutdown() {
  if (isStopping) {
    return;
  }

  isStopping = true;
  clearInterval(pollTimer);
  await stopBackend();
  process.exit(0);
}

async function stopBackend() {
  if (!backendProcess || backendProcess.exitCode !== null) {
    backendProcess = null;
    return;
  }

  const child = backendProcess;
  await new Promise((resolve) => {
    const timeout = setTimeout(resolve, 5000);
    child.once("exit", () => {
      clearTimeout(timeout);
      resolve();
    });

    if (process.platform === "win32") {
      spawn("taskkill.exe", ["/PID", String(child.pid), "/T", "/F"], {
        stdio: "ignore",
      }).once("exit", () => undefined);
      return;
    }

    try {
      process.kill(-child.pid, "SIGTERM");
    } catch {
      child.kill("SIGTERM");
    }
  });

  backendProcess = null;
}

async function sourceSignature() {
  const records = [];

  for (const entry of watchEntries) {
    await collectFileRecords(path.join(repoRoot, entry), records);
  }

  return records.sort().join("\n");
}

async function collectFileRecords(filePath, records) {
  let metadata;

  try {
    metadata = await stat(filePath);
  } catch {
    return;
  }

  if (metadata.isDirectory()) {
    const entries = await readdir(filePath);

    for (const entry of entries) {
      await collectFileRecords(path.join(filePath, entry), records);
    }

    return;
  }

  if (!metadata.isFile() || !watchedExtensions.has(path.extname(filePath))) {
    return;
  }

  records.push(
    `${path.relative(repoRoot, filePath)}:${metadata.mtimeMs}:${metadata.size}`,
  );
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}
