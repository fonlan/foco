import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { existsSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig, type Plugin } from "vitest/config";

const backendEndpoint = loadBackendEndpoint();
const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

export default defineConfig({
  plugins: [react(), tailwindcss(), focoIconPlugin(), backendReloadPlugin()],
  server: {
    proxy: {
      "^/api(?:/|$|\\?)": {
        target: backendEndpoint.origin,
        ws: true,
      },
    },
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: "./test-setup.ts",
  },
});

function focoIconPlugin(): Plugin {
  const iconPath = resolve(repoRoot, "foco.svg");

  return {
    name: "foco-icon",
    configureServer(server) {
      server.watcher.add(iconPath);
      server.middlewares.use((request, response, next) => {
        if (request.url?.split("?")[0] !== "/foco.svg") {
          next();
          return;
        }

        try {
          response.statusCode = 200;
          response.setHeader("Content-Type", "image/svg+xml");
          response.setHeader("Cache-Control", "no-cache");
          response.end(readFileSync(iconPath));
        } catch (error) {
          next(error as Error);
        }
      });
    },
    generateBundle() {
      this.emitFile({
        type: "asset",
        fileName: "foco.svg",
        source: readFileSync(iconPath, "utf8"),
      });
    },
  };
}

function backendReloadPlugin(): Plugin {
  const backendHealthUrl = `${backendEndpoint.origin}/api/health`;
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

function loadBackendEndpoint() {
  const saved = readSavedBackendEndpoint();
  const host = process.env.FOCO_HOST ?? saved.host ?? "127.0.0.1";
  const port = process.env.FOCO_PORT ?? saved.port ?? "3210";

  return {
    origin: `http://${formatHostForUrl(connectHostForListenHost(host))}:${port}`,
  };
}

function readSavedBackendEndpoint() {
  const configDir = process.env.FOCO_CONFIG_DIR ?? defaultConfigDir();

  if (!configDir) {
    return {};
  }

  const configPath = resolve(configDir, "config.json");

  if (!existsSync(configPath)) {
    return {};
  }

  const config = JSON.parse(readFileSync(configPath, "utf8"));
  const webServer = config.app?.web_server;

  return {
    host: typeof webServer?.listen_host === "string" ? webServer.listen_host : undefined,
    port: typeof webServer?.listen_port === "number" ? String(webServer.listen_port) : undefined,
  };
}

function defaultConfigDir() {
  const userProfile = process.env.USERPROFILE ?? process.env.HOME;
  return userProfile ? resolve(userProfile, ".foco") : undefined;
}

function formatHostForUrl(host: string) {
  if (host.includes(":") && !host.startsWith("[")) {
    return `[${host}]`;
  }

  return host;
}

function connectHostForListenHost(host: string) {
  if (host === "0.0.0.0") {
    return "127.0.0.1";
  }

  if (host === "::" || host === "[::]") {
    return "::1";
  }

  return host;
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
