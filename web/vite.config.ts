import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig, type Plugin } from "vite";

const backendPort = process.env.FOCO_PORT ?? "3210";
const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

export default defineConfig({
  plugins: [react(), tailwindcss(), backendReloadPlugin()],
  server: {
    proxy: {
      "/api": `http://127.0.0.1:${backendPort}`,
    },
  },
});

function backendReloadPlugin(): Plugin {
  const backendHealthUrl = `http://127.0.0.1:${backendPort}/api/health`;
  let reloadSequence = 0;
  let reloadTimer: ReturnType<typeof setTimeout> | null = null;

  return {
    name: "foco-backend-reload",
    configureServer(server) {
      server.watcher.add([
        resolve(repoRoot, "Cargo.lock"),
        resolve(repoRoot, "Cargo.toml"),
        resolve(repoRoot, "agent/**/*.rs"),
        resolve(repoRoot, "agent/**/*.toml"),
        resolve(repoRoot, "app/**/*.rs"),
        resolve(repoRoot, "app/**/*.toml"),
        resolve(repoRoot, "graph/**/*.rs"),
        resolve(repoRoot, "graph/**/*.toml"),
        resolve(repoRoot, "mcp/**/*.rs"),
        resolve(repoRoot, "mcp/**/*.toml"),
        resolve(repoRoot, "providers/**/*.rs"),
        resolve(repoRoot, "providers/**/*.toml"),
        resolve(repoRoot, "store/**/*.rs"),
        resolve(repoRoot, "store/**/*.toml"),
        resolve(repoRoot, "tools/**/*.rs"),
        resolve(repoRoot, "tools/**/*.toml"),
      ]);

      server.watcher.on("change", (filePath) => {
        if (!isBackendFile(filePath)) {
          return;
        }

        reloadSequence += 1;

        if (reloadTimer) {
          clearTimeout(reloadTimer);
        }

        const sequence = reloadSequence;
        reloadTimer = setTimeout(() => {
          void reloadWhenBackendIsHealthy(backendHealthUrl, sequence, () => {
            return sequence === reloadSequence;
          }).then((shouldReload) => {
            if (shouldReload) {
              server.ws.send({ type: "full-reload" });
            }
          });
        }, 1500);
      });
    },
  };
}

async function reloadWhenBackendIsHealthy(
  healthUrl: string,
  sequence: number,
  isCurrent: () => boolean,
) {
  const deadline = Date.now() + 60_000;

  while (Date.now() < deadline) {
    if (!isCurrent()) {
      return false;
    }

    if (await isBackendHealthy(healthUrl)) {
      return sequence > 0;
    }

    await delay(500);
  }

  return false;
}

async function isBackendHealthy(healthUrl: string) {
  try {
    const response = await fetch(healthUrl, { cache: "no-store" });
    return response.ok;
  } catch {
    return false;
  }
}

function delay(milliseconds: number) {
  return new Promise((resolve) => {
    setTimeout(resolve, milliseconds);
  });
}

function isBackendFile(filePath: string) {
  const normalized = filePath.replaceAll("\\", "/");

  return (
    normalized.endsWith("/Cargo.lock") ||
    normalized.endsWith("/Cargo.toml") ||
    /\/(agent|app|graph|mcp|providers|store|tools)\/.*\.(rs|toml)$/.test(
      normalized,
    )
  );
}
