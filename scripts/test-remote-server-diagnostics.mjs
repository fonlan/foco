#!/usr/bin/env node
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";

const repoRoot = path.resolve(import.meta.dirname, "..");
const remoteServersRs = await readFile(
  path.join(repoRoot, "app/http/remote_servers.rs"),
  "utf8",
);
const remoteWorkspaceRs = await readFile(
  path.join(repoRoot, "app/remote_workspace.rs"),
  "utf8",
);
const settingsPanelTsx = await readFile(
  path.join(repoRoot, "web/features/settings/SettingsPanel.tsx"),
  "utf8",
);
const workspaceDialogTsx = await readFile(
  path.join(repoRoot, "web/features/workspaces/WorkspaceDialog.tsx"),
  "utf8",
);
const appTsx = await readFile(path.join(repoRoot, "web/App.tsx"), "utf8");
const i18nTs = await readFile(path.join(repoRoot, "web/shared/i18n.ts"), "utf8");

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
assert.match(remoteServersRs, /remote_server_summary_sidecar_install_state/, "summary derives sidecar install state");
assert.match(remoteServersRs, /server_summary_treats_ready_version_as_available_sidecar/, "stale notInstalled with version is covered");
assert.match(remoteWorkspaceRs, /"\.foco\/sidecars\/\{\}\/\{\}"/, "sidecar install directory is built under $HOME/.foco");
assert.match(remoteWorkspaceRs, /format!\("\\\"\$HOME\\\"\/\{\}"/, "remote home path keeps $HOME expandable");
assert.doesNotMatch(remoteWorkspaceRs, /format!\(\s*"~\/\.foco\/sidecars/, "sidecar install path does not use literal tilde");
assert.match(remoteWorkspaceRs, /session_path=\\"\$dir\//, "session script uses session_path variable");
assert.doesNotMatch(remoteWorkspaceRs, /; path=\\"\$dir\//, "session script does not assign zsh path variable");

for (const source of [settingsPanelTsx, workspaceDialogTsx, appTsx]) {
  assert.match(source, /Checking Sidecar version/, "focoCommandVersion label checks sidecar version");
}
assert.match(i18nTs, /"Checking Sidecar version": "检查 Sidecar 版本"/, "Chinese diagnostic label is translated");
assert.doesNotMatch(settingsPanelTsx, /case "focoCommandVersion":[\s\S]{0,80}Syncing config/, "settings diagnostic label is not config sync");
assert.doesNotMatch(workspaceDialogTsx, /case "focoCommandVersion":[\s\S]{0,80}Syncing config/, "workspace diagnostic label is not config sync");
assert.match(settingsPanelTsx, /const isConnected = server\.status === "connected" \|\| server\.status === "ready";/, "connected and ready servers use disconnect state");
assert.match(settingsPanelTsx, /icon=\{isConnected \? CircleAlert : Play\}/, "one toggle button switches icon");
assert.match(settingsPanelTsx, /onClick=\{\(\) => void onRunOperation\(server, toggleOperation\)\}/, "one toggle button switches operation");

console.log("remote server diagnostics self-check passed");
