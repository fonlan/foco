#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import {
  chmod,
  copyFile,
  cp,
  mkdir,
  mkdtemp,
  rm,
  symlink,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import process from "node:process";

const APP_NAME = "Foco";
const BUNDLE_ID = "app.foco.Foco";
const EXECUTABLE_NAME = "foco";
const repoRoot = path.resolve(import.meta.dirname, "..");
const distRoot = path.join(repoRoot, "dist", "macos");
const appRoot = path.join(distRoot, `${APP_NAME}.app`);
const contentsDir = path.join(appRoot, "Contents");
const macosDir = path.join(contentsDir, "MacOS");
const resourcesDir = path.join(contentsDir, "Resources");
const iconPath = path.join(resourcesDir, `${APP_NAME}.icns`);
const dmgPath = path.join(distRoot, `${APP_NAME}.dmg`);

try {
  const options = parseArgs(process.argv.slice(2));
  assertMacosHost();

  runNpm(["run", "build", "-w", "web"]);
  runCargo(["build", "--release", "-p", "foco-app"]);

  await buildAppBundle();

  if (options.dmg) {
    await buildDmg();
  }

  console.log(`[macos] packaged ${appRoot}`);
  if (options.dmg) {
    console.log(`[macos] packaged ${dmgPath}`);
  }
} catch (error) {
  console.error(`[macos] ${errorMessage(error)}`);
  process.exitCode = 1;
}

function parseArgs(args) {
  const unknown = args.filter((arg) => arg !== "--dmg");
  if (unknown.length > 0) {
    throw new Error(`unknown argument: ${unknown[0]}`);
  }

  return {
    dmg: args.includes("--dmg"),
  };
}

function assertMacosHost() {
  if (process.platform !== "darwin") {
    throw new Error("macOS app packaging must run on macOS.");
  }
}

async function buildAppBundle() {
  const targetRoot = path.resolve(repoRoot, process.env.CARGO_TARGET_DIR ?? "target");
  const releaseExecutable = path.join(targetRoot, "release", EXECUTABLE_NAME);

  if (!existsSync(releaseExecutable)) {
    throw new Error(`release executable was not created: ${releaseExecutable}`);
  }

  await rm(appRoot, { force: true, recursive: true });
  await mkdir(macosDir, { recursive: true });
  await mkdir(resourcesDir, { recursive: true });

  const bundledExecutable = path.join(macosDir, EXECUTABLE_NAME);
  await copyFile(releaseExecutable, bundledExecutable);
  await chmod(bundledExecutable, 0o755);

  await writeIcns();
  await writeInfoPlist(await cargoPackageVersion());
}

async function writeIcns() {
  const sourceSvg = path.join(repoRoot, "foco.svg");
  if (!existsSync(sourceSvg)) {
    throw new Error(`missing app icon source: ${sourceSvg}`);
  }

  const tempRoot = await mkdtemp(path.join(tmpdir(), "foco-icon-"));
  const iconsetDir = path.join(tempRoot, `${APP_NAME}.iconset`);

  try {
    await mkdir(iconsetDir, { recursive: true });

    const iconFiles = [
      ["icon_16x16.png", 16],
      ["icon_16x16@2x.png", 32],
      ["icon_32x32.png", 32],
      ["icon_32x32@2x.png", 64],
      ["icon_128x128.png", 128],
      ["icon_128x128@2x.png", 256],
      ["icon_256x256.png", 256],
      ["icon_256x256@2x.png", 512],
      ["icon_512x512.png", 512],
      ["icon_512x512@2x.png", 1024],
    ];

    for (const [fileName, size] of iconFiles) {
      runQuiet("sips", [
        "-s",
        "format",
        "png",
        "-z",
        String(size),
        String(size),
        sourceSvg,
        "--out",
        path.join(iconsetDir, fileName),
      ]);
    }

    run("iconutil", ["-c", "icns", "-o", iconPath, iconsetDir]);
  } finally {
    await rm(tempRoot, { force: true, recursive: true });
  }
}

async function writeInfoPlist(version) {
  const plist = `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>${escapePlist(APP_NAME)}</string>
  <key>CFBundleDisplayName</key>
  <string>${escapePlist(APP_NAME)}</string>
  <key>CFBundleIdentifier</key>
  <string>${escapePlist(BUNDLE_ID)}</string>
  <key>CFBundleVersion</key>
  <string>${escapePlist(version)}</string>
  <key>CFBundleShortVersionString</key>
  <string>${escapePlist(version)}</string>
  <key>CFBundleExecutable</key>
  <string>${escapePlist(EXECUTABLE_NAME)}</string>
  <key>CFBundleIconFile</key>
  <string>${escapePlist(APP_NAME)}</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>LSMinimumSystemVersion</key>
  <string>13.0</string>
  <key>LSUIElement</key>
  <true/>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
`;

  await writeFile(path.join(contentsDir, "Info.plist"), plist, "utf8");
}

async function buildDmg() {
  const stagingRoot = path.join(distRoot, "dmg-root");

  await rm(dmgPath, { force: true });
  await rm(stagingRoot, { force: true, recursive: true });
  await mkdir(stagingRoot, { recursive: true });

  try {
    await cp(appRoot, path.join(stagingRoot, `${APP_NAME}.app`), {
      recursive: true,
    });
    await symlink("/Applications", path.join(stagingRoot, "Applications"));
    run("hdiutil", [
      "create",
      "-volname",
      APP_NAME,
      "-srcfolder",
      stagingRoot,
      "-ov",
      "-format",
      "UDZO",
      dmgPath,
    ]);
  } finally {
    await rm(stagingRoot, { force: true, recursive: true });
  }
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

function runNpm(args) {
  run("npm", args);
}

function runCargo(args) {
  run("cargo", args, {
    MACOSX_DEPLOYMENT_TARGET: process.env.MACOSX_DEPLOYMENT_TARGET ?? "13.0",
  });
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

function runQuiet(command, args, extraEnv = {}) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    env: { ...process.env, ...extraEnv },
    stdio: "ignore",
    windowsHide: true,
  });

  if (result.error) {
    throw result.error;
  }

  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} exited with code ${result.status}`);
  }
}

function escapePlist(value) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&apos;");
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}
