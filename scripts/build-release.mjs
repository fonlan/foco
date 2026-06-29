import { spawnSync } from "node:child_process";
import path from "node:path";
import process from "node:process";

const repoRoot = path.resolve(import.meta.dirname, "..");

try {
  const options = parseArgs(process.argv.slice(2));
  const executable = assertTargetNotRunning(options.profile);

  if (options.checkOnly) {
    console.log(`[release] ${options.profile} preflight passed: ${executable}`);
  } else {
    runNpm(["run", "build", "-w", "web"]);
    runCargo(
      options.profile === "dist"
        ? ["build", "--profile", "dist", "-p", "foco-app"]
        : ["build", "--release", "-p", "foco-app"],
    );
  }
} catch (error) {
  console.error(`[release] ${errorMessage(error)}`);
  process.exitCode = 1;
}

function parseArgs(args) {
  const unknown = args.filter(
    (arg) => arg !== "--dist" && arg !== "--check-only",
  );

  if (unknown.length > 0) {
    throw new Error(`unknown argument: ${unknown[0]}`);
  }

  return {
    checkOnly: args.includes("--check-only"),
    profile: args.includes("--dist") ? "dist" : "release",
  };
}

function assertTargetNotRunning(profile) {
  const targetRoot = path.resolve(
    repoRoot,
    process.env.CARGO_TARGET_DIR ?? "target",
  );
  const executable = path.join(
    targetRoot,
    profile,
    process.platform === "win32" ? "foco.exe" : "foco",
  );

  if (process.platform !== "win32") {
    return executable;
  }

  const command = [
    "$target = [IO.Path]::GetFullPath($env:FOCO_BUILD_TARGET_EXE)",
    "$running = Get-Process foco -ErrorAction SilentlyContinue | Where-Object { $_.Path -and [IO.Path]::GetFullPath($_.Path) -eq $target }",
    "if ($running) { exit 1 }",
    "exit 0",
  ].join("; ");

  const result = spawnSync(
    "powershell.exe",
    ["-NoProfile", "-NonInteractive", "-Command", command],
    {
      cwd: repoRoot,
      env: { ...process.env, FOCO_BUILD_TARGET_EXE: executable },
      stdio: "ignore",
      windowsHide: true,
    },
  );

  if (result.error) {
    throw result.error;
  }

  if (result.status !== 0) {
    throw new Error(
      `release executable is running: ${executable}. Exit Foco and retry.`,
    );
  }

  return executable;
}

function runNpm(args) {
  if (process.platform === "win32") {
    run("cmd.exe", ["/d", "/s", "/c", "npm.cmd", ...args]);
    return;
  }

  run("npm", args);
}

function runCargo(args) {
  run(process.platform === "win32" ? "cargo.exe" : "cargo", args);
}

function run(command, args, extraEnv = {}) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    env: { ...process.env, ...extraEnv },
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
