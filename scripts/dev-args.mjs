const BACKEND_PORT_FLAGS = new Set(["--port", "--backend-port"]);
const BACKEND_HOST_FLAGS = new Set(["--host", "--backend-host"]);

export function parseBackendDevArgs(args, sourceEnv = {}) {
  const env = {};

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];

    if (arg === "--") {
      continue;
    }

    const name = optionName(arg);

    if (BACKEND_PORT_FLAGS.has(name)) {
      const parsed = readOptionValue(args, index, name);
      env.FOCO_PORT = normalizePort(parsed.value, name);
      index = parsed.index;
      continue;
    }

    if (BACKEND_HOST_FLAGS.has(name)) {
      const parsed = readOptionValue(args, index, name);
      env.FOCO_HOST = requireValue(parsed.value, name);
      index = parsed.index;
      continue;
    }

    if (name === "--config-dir") {
      const parsed = readOptionValue(args, index, name);
      env.FOCO_CONFIG_DIR = requireValue(parsed.value, name);
      index = parsed.index;
      continue;
    }

    if (!arg.startsWith("--")) {
      applyBackendPositional(env, arg);
      continue;
    }

    throw new Error(`unknown backend option '${arg}'`);
  }

  applyBackendNpmConfig(env, sourceEnv);
  return { env };
}

export function parseFrontendDevArgs(args, sourceEnv = {}) {
  const env = {};
  const viteArgs = [];

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];

    if (arg === "--") {
      continue;
    }

    const name = optionName(arg);

    if (name === "--backend-port") {
      const parsed = readOptionValue(args, index, name);
      env.FOCO_PORT = normalizePort(parsed.value, name);
      index = parsed.index;
      continue;
    }

    if (name === "--backend-host") {
      const parsed = readOptionValue(args, index, name);
      env.FOCO_HOST = requireValue(parsed.value, name);
      index = parsed.index;
      continue;
    }

    if (name === "--config-dir") {
      const parsed = readOptionValue(args, index, name);
      env.FOCO_CONFIG_DIR = requireValue(parsed.value, name);
      index = parsed.index;
      continue;
    }

    if (arg.startsWith("--")) {
      viteArgs.push(arg);

      if (optionName(arg) === arg && args[index + 1] && !args[index + 1].startsWith("--")) {
        viteArgs.push(args[index + 1]);
        index += 1;
      }

      continue;
    }

    applyFrontendPositional(env, viteArgs, arg);
  }

  applyFrontendNpmConfig(env, viteArgs, sourceEnv);
  return { env, viteArgs };
}

export function formatDevEnvOverrides(env) {
  const entries = Object.entries(env);

  if (entries.length === 0) {
    return "";
  }

  return ` with ${entries.map(([key, value]) => `${key}=${value}`).join(" ")}`;
}

export function cleanDevEnv(sourceEnv, overrides = {}) {
  const env = {};

  for (const [key, value] of Object.entries(sourceEnv)) {
    if (!isValidEnvEntry(key, value)) {
      continue;
    }

    env[key] = value;
  }

  for (const [key, value] of Object.entries(overrides)) {
    if (!isValidEnvEntry(key, value)) {
      throw new Error(`invalid environment override '${key}'`);
    }

    env[key] = value;
  }

  return env;
}

function optionName(arg) {
  const equalsIndex = arg.indexOf("=");
  return equalsIndex === -1 ? arg : arg.slice(0, equalsIndex);
}

function readOptionValue(args, index, name) {
  const arg = args[index];
  const equalsIndex = arg.indexOf("=");

  if (equalsIndex !== -1) {
    return { value: arg.slice(equalsIndex + 1), index };
  }

  const value = args[index + 1];

  if (value === undefined || value.startsWith("--")) {
    throw new Error(`${name} requires a value`);
  }

  return { value, index: index + 1 };
}

function requireValue(value, name) {
  const trimmed = value.trim();

  if (!trimmed) {
    throw new Error(`${name} requires a non-empty value`);
  }

  return trimmed;
}

function normalizePort(value, name) {
  const trimmed = requireValue(value, name);

  if (!/^\d+$/.test(trimmed)) {
    throw new Error(`${name} must be a number from 1 to 65535`);
  }

  const port = Number(trimmed);

  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    throw new Error(`${name} must be a number from 1 to 65535`);
  }

  return String(port);
}

function isPort(value) {
  return /^\d+$/.test(value.trim());
}

function applyBackendPositional(env, value) {
  if (!env.FOCO_PORT && isPort(value)) {
    env.FOCO_PORT = normalizePort(value, "backend port");
    return;
  }

  if (!env.FOCO_CONFIG_DIR) {
    env.FOCO_CONFIG_DIR = requireValue(value, "config directory");
    return;
  }

  throw new Error(`unknown backend option '${value}'`);
}

function applyFrontendPositional(env, viteArgs, value) {
  if (!env.FOCO_PORT && isPort(value)) {
    env.FOCO_PORT = normalizePort(value, "backend port");
    return;
  }

  if (!env.FOCO_CONFIG_DIR) {
    env.FOCO_CONFIG_DIR = requireValue(value, "config directory");
    return;
  }

  if (isPort(value) && !viteArgs.some((arg) => arg === "--port" || arg.startsWith("--port="))) {
    viteArgs.push(`--port=${normalizePort(value, "frontend port")}`);
    return;
  }

  viteArgs.push(value);
}

function applyBackendNpmConfig(env, sourceEnv) {
  const port = envValue(sourceEnv, ["npm_config_port", "npm_config_backend_port"]);
  const host = envValue(sourceEnv, ["npm_config_host", "npm_config_backend_host"]);
  const configDir = envValue(sourceEnv, ["npm_config_config_dir"]);

  if (!env.FOCO_PORT && port) {
    env.FOCO_PORT = normalizePort(port, "backend port");
  }

  if (!env.FOCO_HOST && host) {
    env.FOCO_HOST = requireValue(host, "backend host");
  }

  if (!env.FOCO_CONFIG_DIR && configDir) {
    env.FOCO_CONFIG_DIR = requireValue(configDir, "config directory");
  }
}

function applyFrontendNpmConfig(env, viteArgs, sourceEnv) {
  const backendPort = envValue(sourceEnv, ["npm_config_backend_port"]);
  const backendHost = envValue(sourceEnv, ["npm_config_backend_host"]);
  const configDir = envValue(sourceEnv, ["npm_config_config_dir"]);
  const frontendPort = envValue(sourceEnv, ["npm_config_port"]);

  if (!env.FOCO_PORT && backendPort) {
    env.FOCO_PORT = normalizePort(backendPort, "backend port");
  }

  if (!env.FOCO_HOST && backendHost) {
    env.FOCO_HOST = requireValue(backendHost, "backend host");
  }

  if (!env.FOCO_CONFIG_DIR && configDir) {
    env.FOCO_CONFIG_DIR = requireValue(configDir, "config directory");
  }

  if (
    frontendPort &&
    !viteArgs.some((arg) => arg === "--port" || arg.startsWith("--port="))
  ) {
    viteArgs.push(`--port=${normalizePort(frontendPort, "frontend port")}`);
  }
}

function envValue(sourceEnv, names) {
  const normalized = new Map(
    Object.entries(sourceEnv).map(([key, value]) => [key.toLowerCase(), value]),
  );

  for (const name of names) {
    const value = normalized.get(name.toLowerCase());

    if (typeof value === "string" && value.trim()) {
      return value;
    }
  }

  return undefined;
}

function isValidEnvEntry(key, value) {
  return (
    key &&
    !key.startsWith("-") &&
    !key.includes("=") &&
    !key.includes("\0") &&
    typeof value === "string" &&
    !value.includes("\0")
  );
}
