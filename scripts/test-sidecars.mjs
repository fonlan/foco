#!/usr/bin/env node
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import process from "node:process";

const repoRoot = path.resolve(import.meta.dirname, "..");
const tempRoot = await mkdtemp(path.join(tmpdir(), "foco-sidecars-test-"));
const binary = path.join(tempRoot, "foco");
const sidecars = path.join(tempRoot, "sidecars");
const copied = path.join(tempRoot, "copied-sidecars");

try {
  await writeFile(binary, "fake sidecar\n", "utf8");

  runSidecars([
    "manifest",
    "--target",
    "linux-x64",
    "--binary",
    binary,
    "--out",
    sidecars,
    "--version",
    "0.0.0-test",
  ]);
  runSidecars(["verify", "--root", sidecars]);
  runSidecars(["copy", "--source", sidecars, "--dest", copied]);

  const manifest = JSON.parse(await readFile(path.join(copied, "manifest.json"), "utf8"));
  assert.equal(manifest.version, "0.0.0-test");
  assert.deepEqual(manifest.sidecars.map((entry) => entry.target), ["linux-x64"]);
  assert.equal(manifest.sidecars[0].path, "linux-x64/foco");
  assert.ok(existsSync(path.join(copied, ...manifest.sidecars[0].path.split("/"))));

  await writeFile(path.join(copied, ...manifest.sidecars[0].path.split("/")), "tampered\n", "utf8");
  const failed = spawnSidecars(["verify", "--root", copied]);
  assert.notEqual(failed.status, 0);
  assert.match(failed.stderr, /sha256 mismatch/);

  console.log("sidecars script self-check passed");
} finally {
  await rm(tempRoot, { force: true, recursive: true });
}

function runSidecars(args) {
  const result = spawnSidecars(args);
  if (result.status !== 0) {
    throw new Error(`sidecars ${args.join(" ")} failed: ${result.stderr}`);
  }
  return result;
}

function spawnSidecars(args) {
  const result = spawnSync(process.execPath, ["scripts/sidecars.mjs", ...args], {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  if (result.error) {
    throw result.error;
  }
  return result;
}
