#!/usr/bin/env node
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";

const repoRoot = path.resolve(import.meta.dirname, "..");
const remoteServersRs = await readFile(
  path.join(repoRoot, "app/http/remote_servers.rs"),
  "utf8",
);

const requiredDiagnosticStages = [
  "ssh",
  "target",
  "sidecarAsset",
  "remoteInstallDirWritable",
  "focoCommandVersion",
];
for (const stage of requiredDiagnosticStages) {
  assert.match(remoteServersRs, new RegExp(`pending_stage\\("${stage}"\\)`), `${stage} stage exists`);
}

const requiredErrorKinds = [
  "authentication_failed",
  "host_unreachable",
  "target_unsupported",
  "permission_denied",
  "sidecar_asset_missing",
  "startup_failed",
];
for (const kind of requiredErrorKinds) {
  assert.match(remoteServersRs, new RegExp(`"${kind}"`), `${kind} diagnostic kind exists`);
}

assert.match(remoteServersRs, /select_sidecar_asset\(&target\)/, "diagnostics select packaged sidecar by target");
assert.match(remoteServersRs, /Sha256::digest\(&bytes\)/, "sidecar selection verifies sha256");
assert.match(remoteServersRs, /sidecar asset sha256 mismatch/, "sha256 mismatch is reported");
assert.match(remoteServersRs, /BatchMode=yes/, "SSH diagnostics use BatchMode");
assert.match(remoteServersRs, /mkdir -p ~\/\.foco\/sidecars && test -w ~\/\.foco\/sidecars/, "install dir writability is checked");

console.log("remote server diagnostics self-check passed");
