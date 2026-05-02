# AGENT.md — Codex operating manual for Stream

## What you are
An autonomous coding agent for "Stream", a Windows-only premium P2P remote desktop app written in Rust + Tauri 2. Target latency 1–3 ms LAN, 10–25 ms WAN. Reference SKILL.md for full architectural rules; this file is the operational handbook.

## Repository layout
- `crates/engine` — pure Rust, no Tauri. Capture, encode, network, decode, render. Sub-crates: `capture-dxgi`, `capture-wgc`, `encode-nvenc`, `encode-amf`, `encode-qsv`, `transport-quic`, `pairing-noise`, `input`, `audio`.
- `crates/protocol` — frame protocol, FEC, control RPC schemas (prost-generated).
- `crates/ipc` — named-pipe JSON-RPC between engine and UI.
- `crates/host-service` — Windows service wrapper (SYSTEM context for UAC capture).
- `app/` — Tauri 2 + Next.js UI. `app/src-tauri` Rust shell, `app/src` React.
- `signaling/` — Cloudflare Worker + Durable Object (TypeScript, `workers-rs` optional).
- `infra/` — Terraform/Pulumi for Oracle Free Tier coturn, Cloudflare DNS.
- `installer/` — Tauri bundler config, WiX project, NSIS scripts, signing pipeline.
- `tools/` — latency measurement harness, FEC profiler, capture replay.

## Command reference
- `cargo build --release` — build engine.
- `cargo test --workspace` — unit + integration tests.
- `cargo run -p latency-bench -- --resolution 1920x1080 --fps 144` — measure capture+encode budget.
- `pnpm tauri dev` — develop UI locally.
- `pnpm tauri build --target x86_64-pc-windows-msvc` — produce signed installer (signing requires Azure Trusted Signing creds in env).
- `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings` — pre-commit gate.
- `cargo deny check` — license/advisory audit.
- `cargo +nightly miri test -p protocol` — UB check on protocol parser.

## Definition of done for any change
1. Compiles on `stable` Rust pinned in rust-toolchain.toml (currently 1.86).
2. `cargo clippy -D warnings` clean.
3. `cargo test --workspace` green.
4. Latency benchmark shows no regression beyond 5% on `cargo run -p latency-bench`.
5. If touching `crates/protocol`, the wire-format compatibility test in `tests/protocol-compat.rs` passes against the previous tagged release.
6. If touching the encoder, `tools/encoder-conformance --codec h264 --codec hevc --codec av1` exits 0.
7. UI changes include a Storybook entry and pass `pnpm test:visual` (Playwright + Percy).
8. Signed installer artifact builds in CI on `main` branch.

## Coding conventions
- Use `anyhow::Result` at app boundaries, `thiserror` enums in libraries.
- All `unsafe` blocks must have a `// SAFETY:` comment naming the invariant.
- `tracing::instrument` on every hot-path-adjacent function.
- Public functions with documented latency contracts annotate `// LAT: <bound>` (e.g. `// LAT: ≤1 ms p99 on RTX 4070`).
- No `unwrap()` in non-test code except behind `expect()` with a reason.
- All FFI to vendor SDKs (NVENC, AMF, QSV) goes through a thin `*-sys` crate then a safe wrapper crate; no `unsafe` outside the wrapper.
- SPSC ring buffers between threads. No `Arc<Mutex<T>>` on the hot path.
- Atomics that two threads touch are wrapped in `CachePadded`.

## How to add a new feature
1. Re-read SKILL.md "hard rules". If your feature violates one, stop and surface the conflict.
2. State explicitly: which process, which thread, which ring buffer, which control message.
3. Sketch the wire-protocol delta (new RPC, new datagram type, new field). Update `crates/protocol` first, regenerate prost, write a backwards-compat test.
4. Implement engine side; add `tracing::instrument` and a `criterion` benchmark if it touches the hot path.
5. Implement IPC RPC in `crates/ipc`.
6. Implement UI side in `app/src`.
7. Add E2E test in `tests/e2e/` running engine and a mock client through the loopback adapter.

## Loopback test harness
`tools/loopback` runs a host engine and a client engine in the same process talking over a `tokio::net::UnixDatagram` (Windows: `tokio::net::UnixStream` since recent tokio supports it via AF_UNIX) so latency tests are deterministic. Use this for protocol changes and FEC/CC tuning.

## When stuck
1. Re-read SKILL.md hard rules.
2. Search the relevant vendor SDK doc URL in `docs/references.md`.
3. Check Sunshine source at `vendor/sunshine-reference/` (vendored as git subtree at a known commit) for prior art.
4. Open a draft PR with a `// FIXME(agent):` comment explaining the blocker.

## Things you must never do without explicit human approval
- Modify the wire protocol in a backwards-incompatible way.
- Add a new vendor SDK dependency.
- Add `Arc<Mutex<_>>` to the hot path.
- Use `unsafe` outside a `*-sys` wrapper crate.
- Disable a clippy lint repo-wide.
- Bump tokio to multi-threaded runtime in the engine.
- Replace Quinn with another transport.
- Add an Electron dependency (or anything that pulls Chromium beyond WebView2).
- Sign release binaries from a developer machine — only the CI signing pipeline.
- Commit secrets, certificates, or signing tokens.

## Telemetry and privacy
Telemetry is opt-in, off by default. Schema lives in `crates/protocol/telemetry.proto`. Aggregate latency, FPS, codec, GPU vendor, NAT type, OS build only. No screen content, no clipboard, no input events ever leave the device.

## Release flow
1. Bump version in `Cargo.toml` and `app/package.json`. Tag `vX.Y.Z`.
2. CI runs full test matrix + signs + uploads NSIS installer + WiX MSI to S3.
3. CI generates Tauri updater manifest signed with Ed25519 release key (HSM-stored, GitHub Actions OIDC into AWS).
4. Staged rollout: 1% / 10% / 50% / 100% via Tauri updater channels (`--channel canary|stable`).
5. Crash dashboards (Sentry) checked at each gate.