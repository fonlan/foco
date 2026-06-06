import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import process from "node:process";

if (process.platform !== "win32") {
  throw new Error("Windows release smoke test must run on Windows.");
}

const repoRoot = path.resolve(import.meta.dirname, "..");
const releaseExe = path.join(repoRoot, "target", "release", "foco.exe");
const profileDir = await mkdtemp(path.join(tmpdir(), "foco-release-smoke-"));
const port = String(await freePort());
let appProcess = null;
let isStopping = false;

try {
  await run("cmd.exe", ["/d", "/s", "/c", "npm.cmd", "run", "build:release"]);

  if (!existsSync(releaseExe)) {
    throw new Error(`release executable was not created: ${releaseExe}`);
  }

  appProcess = spawn(releaseExe, [], {
    cwd: repoRoot,
    detached: false,
    env: {
      ...process.env,
      FOCO_PORT: port,
      USERPROFILE: profileDir,
    },
    stdio: "ignore",
    windowsHide: true,
  });

  appProcess.once("exit", (code, signal) => {
    appProcess = null;
    if (isStopping) {
      return;
    }

    if (code !== null && code !== 0) {
      console.error(`foco.exe exited during smoke test with code ${code}`);
    } else if (signal) {
      console.error(`foco.exe exited during smoke test from signal ${signal}`);
    }
  });

  await waitForHealth(`http://127.0.0.1:${port}/api/health`);
  await waitForUi(`http://127.0.0.1:${port}/`);
  await assertFirstRunFiles(profileDir);
} finally {
  if (appProcess?.pid) {
    isStopping = true;
    await killProcessTree(appProcess.pid);
  }

  await rm(profileDir, { force: true, recursive: true });
}

async function assertFirstRunFiles(userProfileDir) {
  const rootDir = path.join(userProfileDir, ".foco");
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

  const today = new Date();
  const logFile = path.join(
    logDir,
    `foco-${today.getFullYear()}-${String(today.getMonth() + 1).padStart(2, "0")}-${String(today.getDate()).padStart(2, "0")}.log`,
  );

  if (!existsSync(logFile)) {
    throw new Error(`first startup did not create today's log file: ${logFile}`);
  }

  console.log(`Windows release smoke passed on http://127.0.0.1:${port}`);
}

async function waitForHealth(url) {
  const deadline = Date.now() + 60_000;
  let lastError = "";

  while (Date.now() < deadline) {
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

  throw new Error(`Foco release health check did not pass: ${lastError}`);
}

async function waitForUi(url) {
  const deadline = Date.now() + 60_000;
  let lastError = "";

  while (Date.now() < deadline) {
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

  throw new Error(`Foco release UI check did not pass: ${lastError}`);
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

async function run(command, args) {
  await new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: repoRoot,
      stdio: ["ignore", "pipe", "pipe"],
      windowsHide: true,
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

async function killProcessTree(pid) {
  await new Promise((resolve) => {
    spawn("taskkill.exe", ["/PID", String(pid), "/T", "/F"], {
      stdio: "ignore",
      windowsHide: true,
    }).once("exit", resolve);
  });
}

function delay(milliseconds) {
  return new Promise((resolve) => {
    setTimeout(resolve, milliseconds);
  });
}
