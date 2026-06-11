import { spawn } from "node:child_process";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import { cleanDevEnv, formatDevEnvOverrides, parseFrontendDevArgs } from "./dev-args.mjs";

const repoRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);
const nodeCommand = process.execPath;
const viteScript = path.join(repoRoot, "node_modules", "vite", "bin", "vite.js");
const webRoot = path.join(repoRoot, "web");

let devOptions;

try {
  devOptions = parseFrontendDevArgs(process.argv.slice(2), process.env);
} catch (error) {
  console.error(`[frontend] ${errorMessage(error)}`);
  console.error(
    "[frontend] usage: npm run frontend -- <backend-port> <config-dir> [frontend-port]",
  );
  process.exit(1);
}
const devEnv = cleanDevEnv(process.env, devOptions.env);

const viteArgs = [viteScript, "--host", "127.0.0.1", ...devOptions.viteArgs];

console.log(
  `[frontend] starting vite ${viteArgs.slice(1).join(" ")}${formatDevEnvOverrides(devOptions.env)}`,
);

const child = spawn(nodeCommand, viteArgs, {
  cwd: webRoot,
  env: devEnv,
  stdio: "inherit",
});

child.once("exit", (code, signal) => {
  if (signal) {
    process.exit(1);
  }

  process.exit(code ?? 0);
});

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}
