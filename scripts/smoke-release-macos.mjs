#!/usr/bin/env node
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import process from "node:process";

class RingBuffer {
  constructor(limit) {
    this.limit = limit;
    this.value = "";
  }

  push(chunk) {
    this.value += chunk.toString("utf8");
    if (this.value.length > this.limit) {
      this.value = this.value.slice(this.value.length - this.limit);
    }
  }

  message() {
    const trimmed = this.value.trim();
    if (!trimmed) {
      return "";
    }

    return `\n\nRecent app output:\n${trimmed}`;
  }
}

if (process.platform !== "darwin") {
  throw new Error("macOS release smoke test must run on macOS.");
}

const repoRoot = path.resolve(import.meta.dirname, "..");
const buildTargetDir = await mkdtemp(path.join(tmpdir(), "foco-macos-release-target-"));
const profileDir = await mkdtemp(path.join(tmpdir(), "foco-macos-release-smoke-"));
const releaseApp = path.join(repoRoot, "dist", "macos", "Foco.app");
const releaseExecutable = path.join(releaseApp, "Contents", "MacOS", "foco");
const port = String(await freePort());
let appProcess = null;
let appExit = null;
let isStopping = false;
const appOutput = new RingBuffer(64_000);

try {
  await run("npm", ["run", "build:macos-app"], {
    CARGO_TARGET_DIR: buildTargetDir,
  });

  if (!existsSync(releaseApp)) {
    throw new Error(`release app bundle was not created: ${releaseApp}`);
  }

  if (!existsSync(releaseExecutable)) {
    throw new Error(`release executable was not created: ${releaseExecutable}`);
  }

  appProcess = spawn(releaseExecutable, [], {
    cwd: repoRoot,
    detached: false,
    env: {
      ...process.env,
      CARGO_TARGET_DIR: buildTargetDir,
      FOCO_PORT: port,
      HOME: profileDir,
    },
    stdio: ["ignore", "pipe", "pipe"],
  });

  appProcess.stdout.on("data", (chunk) => {
    appOutput.push(chunk);
  });
  appProcess.stderr.on("data", (chunk) => {
    appOutput.push(chunk);
  });

  appProcess.once("exit", (code, signal) => {
    appExit = { code, signal };
    appProcess = null;
    if (isStopping) {
      return;
    }

    if (code !== null && code !== 0) {
      console.error(`foco exited during smoke test with code ${code}`);
    } else if (signal) {
      console.error(`foco exited during smoke test from signal ${signal}`);
    }
  });

  await waitForHealth(`http://127.0.0.1:${port}/api/health`);
  await waitForUi(`http://127.0.0.1:${port}/`);
  await assertFirstRunFiles(profileDir);
} finally {
  if (appProcess?.pid) {
    isStopping = true;
    await terminateProcess(appProcess.pid);
  }

  await rm(profileDir, { force: true, recursive: true });
  await rm(buildTargetDir, { force: true, recursive: true });
}

async function assertFirstRunFiles(homeDir) {
  const rootDir = path.join(homeDir, ".foco");
  const configPath = path.join(rootDir, "config.json");
  const workspaceDir = path.join(rootDir, "workspace");
  const logDir = path.join(rootDir, "logs");
  const databasePath = path.join(workspaceDir, ".foco", "foco.sqlite");

  for (const requiredPath of [rootDir, configPath, workspaceDir, logDir, databasePath]) {
    if (!existsSync(requiredPath)) {
      throw new Error(`first startup did not create ${requiredPath}`);
    }
  }

  const config = JSON.parse(await readFile(configPath, "utf8"));
  const defaultWorkspace = config.workspaces?.find(
    (workspace) =>
      workspace.id === config.app?.active_workspace_id &&
      workspace.name === "Default" &&
      path.resolve(workspace.path) === path.resolve(workspaceDir),
  );

  if (!defaultWorkspace) {
    throw new Error("first startup config did not register Default workspace.");
  }

  if (defaultWorkspace.terminal_shell !== "zsh") {
    throw new Error(
      `Default workspace terminal shell should be zsh on macOS, got ${JSON.stringify(defaultWorkspace.terminal_shell)}.`,
    );
  }

  const today = new Date();
  const logFile = path.join(
    logDir,
    `foco-${today.getFullYear()}-${String(today.getMonth() + 1).padStart(2, "0")}-${String(today.getDate()).padStart(2, "0")}.log`,
  );

  if (!existsSync(logFile)) {
    throw new Error(`first startup did not create today's log file: ${logFile}`);
  }

  console.log(`macOS release smoke passed on http://127.0.0.1:${port}`);
}

async function waitForHealth(url) {
  const deadline = Date.now() + 60_000;
  let lastError = "";

  while (Date.now() < deadline) {
    throwIfAppExited();

    try {
      const response = await fetch(url, { cache: "no-store" });
      if (response.ok) {
        const body = await response.json();
        if (body.service === "foco" && body.status === "ok") {
          return;
        }

        lastError = `unexpected health response: ${JSON.stringify(body)}`;
      } else {
        lastError = `${url} returned ${response.status}`;
      }
    } catch (error) {
      lastError = error instanceof Error ? error.message : String(error);
    }

    await delay(500);
  }

  throw new Error(`Foco release health check did not pass: ${lastError}${appOutput.message()}`);
}

async function waitForUi(url) {
  const deadline = Date.now() + 60_000;
  let lastError = "";

  while (Date.now() < deadline) {
    throwIfAppExited();

    try {
      const response = await fetch(url, { cache: "no-store" });
      const html = await response.text();
      if (response.ok && html.includes("<!doctype html") && html.includes("Foco")) {
        return;
      }

      lastError = `${url} returned ${response.status} with unexpected HTML`;
    } catch (error) {
      lastError = error instanceof Error ? error.message : String(error);
    }

    await delay(500);
  }

  throw new Error(`Foco release UI check did not pass: ${lastError}${appOutput.message()}`);
}

function throwIfAppExited() {
  if (!appExit) {
    return;
  }

  const reason =
    appExit.code !== null
      ? `code ${appExit.code}`
      : appExit.signal
        ? `signal ${appExit.signal}`
        : "unknown status";
  throw new Error(`foco exited before smoke test completed with ${reason}.${appOutput.message()}`);
}

async function freePort() {
  const { createServer } = await import("node:net");
  const server = createServer();

  return await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      server.close(() => {
        if (!address || typeof address === "string") {
          reject(new Error("failed to allocate a local TCP port"));
          return;
        }

        resolve(address.port);
      });
    });
  });
}

async function run(command, args, extraEnv = {}) {
  await new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: repoRoot,
      env: {
        ...process.env,
        ...extraEnv,
      },
      stdio: ["ignore", "pipe", "pipe"],
    });

    child.stdout.on("data", (chunk) => {
      process.stdout.write(chunk);
    });
    child.stderr.on("data", (chunk) => {
      process.stderr.write(chunk);
    });
    child.once("error", reject);
    child.once("exit", (code) => {
      if (code === 0) {
        resolve();
        return;
      }

      reject(new Error(`${command} ${args.join(" ")} exited with code ${code}`));
    });
  });
}

async function terminateProcess(pid) {
  try {
    process.kill(pid, "SIGTERM");
  } catch (error) {
    if (error?.code === "ESRCH") {
      return;
    }
    throw error;
  }

  for (let attempt = 0; attempt < 20; attempt += 1) {
    if (!(await processExists(pid))) {
      return;
    }
    await delay(250);
  }

  try {
    process.kill(pid, "SIGKILL");
  } catch (error) {
    if (error?.code !== "ESRCH") {
      throw error;
    }
  }
}

async function processExists(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    if (error?.code === "ESRCH") {
      return false;
    }
    throw error;
  }
}

function delay(milliseconds) {
  return new Promise((resolve) => {
    setTimeout(resolve, milliseconds);
  });
}
