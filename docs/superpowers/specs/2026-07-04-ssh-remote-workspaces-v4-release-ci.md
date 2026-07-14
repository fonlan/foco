# SSH Remote Workspaces v4: Release And CI

Date: 2026-07-04

Scope: Phase 2 makes release builds produce Linux sidecars and bundle them into desktop release artifacts. Runtime upload/install uses the Phase 1 boundary: local Foco owns the packaged asset and uploads it over SSH stdin when a remote workspace first connects.

## CI Sidecar Build

GitHub Actions release workflow builds one `sidecars` artifact on an Ubuntu runner:

- `linux-x64`: `cross build --release -p foco-app --target x86_64-unknown-linux-musl`
- `linux-arm64`: `cross build --release -p foco-app --target aarch64-unknown-linux-musl`

Each build is copied into `sidecars/<target>/foco`. `scripts/sidecars.mjs manifest` writes `sidecars/manifest.json` with:

```json
{
  "version": "0.1.0",
  "sidecars": [
    {
      "target": "linux-arm64",
      "path": "linux-arm64/foco",
      "sha256": "..."
    }
  ]
}
```

`sha256` is always calculated from the copied binary, and release packaging re-verifies every manifest entry before copying.

## musl Default And glibc Fallback

The default is musl because it avoids depending on the remote server's glibc version. Keep musl unless a dependency cannot be linked statically or fails only under musl after normal feature fixes.

Fallback to glibc is allowed only when all of these are true:

- the failing crate or native library is required by the sidecar path,
- there is no reasonable pure-Rust, rustls, bundled, or feature-level fix,
- both `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl` are affected, or the affected architecture is explicitly documented,
- the release workflow target triple changes to the corresponding `*-unknown-linux-gnu` target and the manifest/diagnostics document that this sidecar expects a compatible glibc on the remote host.

The normalized asset names can stay `linux-x64` and `linux-arm64` for the first glibc fallback, but server diagnostics must then include the ABI expectation before upload.

## Desktop Packaging

macOS release jobs download the `sidecars` artifact before `npm run build:macos`. The packaging script copies verified sidecars to:

```text
Foco.app/Contents/Resources/sidecars/
```

Windows release jobs download the same artifact before `npm run build:release -- --bundle-sidecars`. The build script copies verified sidecars to:

```text
target/release/resources/sidecars/
```

A missing manifest or sha mismatch is a hard packaging failure with a hint to build or download the CI sidecar artifact first. Local development builds do not require sidecars unless the explicit bundling/package path is used.

## Runtime Install Contract

Users only install local Foco. For a remote workspace, local Foco selects a packaged sidecar by detected target, verifies `manifest.json`, and uploads it over SSH stdin on first connect. The remote default install path is:

```text
~/.foco/sidecars/<version>/<target>/foco
```

Advanced server profiles may set `focoCommand` to skip upload and run an existing command on the remote server. Diagnostics should still report the command path/version so users can tell whether they are using the packaged sidecar or a custom one.

## Cleanup

Two independent remote lifecycle steps must not be confused:

1. **Orphan process stop (not version-dir cleanup):** before starting a new sidecar for a workspace, Foco may stop already-orphaned remote sidecar **processes** that match the same server/workspace identity. That only signals processes; it does **not** delete old version binaries or directories under `~/.foco/sidecars/`.
2. **Managed version-directory retain (after successful connect):** after a Foco-managed sidecar successfully finishes bootstrap identity verification and the control broker is ready (session registered, workspace Ready), Foco best-effort prunes older **version directories** under remote `~/.foco/sidecars/`.

Version-directory retain policy:

- Root: only direct children of `$HOME/.foco/sidecars/` (no path escape).
- Retain count: **2** total — always protect the currently started managed version directory, then keep one additional recent historical version directory (newest by mtime; equal mtimes break ties by name ascending).
- Candidates: real directories only; skip symlinks and non-directories; unsafe directory names are skipped.
- Trigger: only Foco-managed install paths (`managed_install_version` set). Custom `focoCommand` **does not** run this cleanup.
- Failure mode: best-effort — log warning/diagnostics only; never block opening the workspace or downgrade a successful Ready session.

### Local automatic-update asset retain (desktop main process)

Separately, local auto-update downloads land under `~/.foco/updates/<version>/` on the host that runs Foco. After a successful install restarts the process with `--updated-restart`, the main process best-effort prunes historical version directories under `~/.foco/updates/`, keeping at most **2** (running `CARGO_PKG_VERSION` always protected, plus one recent historical directory by mtime). Ordinary startups do not trigger cleanup. Deletion is limited to validated direct children of the updates root; failures are warning-only and never abort startup.
