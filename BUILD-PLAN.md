# BUILD-PLAN.md — Stream Phase 1 (MVP, weeks 1–8)

Twenty-five atomic tasks that take Stream from empty skeleton to a one-machine end-to-end loopback demo: a SYSTEM-context host process captures the desktop via DDA, encodes with NVENC P1 ULL, ships frames over Quinn QUIC datagrams to a client process, which decodes and renders. A six-digit room code minted by a Cloudflare Worker brokers the connection; a Tauri 2 UI with one Mica-backed connect screen drives the engine over a named pipe.

Out of scope for Phase 1 (deferred to Phase 2): SPAKE2+ pairing, Noise per-session, audio loopback, Reed-Solomon FEC, AMF/QSV/x264 fallbacks, raw-input game mouse, NSIS signing, telemetry, Tauri updater.

## Conventions used in every task

- **Owner.** "Codex 5.5" runs autonomously when the spec is exact and mechanical. "Sonnet 4.6" is invoked interactively when the work is design-sensitive, ambiguous, FFI-heavy, or visual.
- **DoD (Definition of Done).** Every task must satisfy AGENT.md §"Definition of done": pinned `stable` Rust 1.86 compiles, `cargo clippy --workspace --all-targets -- -D warnings` clean, `cargo test --workspace` green, and `cargo run -p latency-bench` shows ≤5% regression once that task exists. Task-specific gates are listed under **Criterion**.
- **Hard rules.** Numbers refer to SKILL.md §"Hard rules (never violate)" 1–10.
- **Pre-commit gate** (every task): `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo deny check`.

---

## Phase 1A — Foundations (tasks 1–5)

### Task 1 — Workspace lint & toolchain gate

- **Owner.** Codex 5.5.
- **Crates / files.** [rust-toolchain.toml](rust-toolchain.toml), [deny.toml](deny.toml), [Cargo.toml](Cargo.toml), [.cargo/config.toml](.cargo/config.toml) (new), [.github/workflows/ci.yml](.github/workflows/ci.yml).
- **Depends on.** None.
- **Hard rules.** None directly; sets up enforcement of all of them.
- **What.** Pin Rust 1.86 stable, add `[lints.workspace]` block forbidding `unsafe_op_in_unsafe_fn`, `clippy::unwrap_used`, `clippy::dbg_macro`, `clippy::todo`. Add `.cargo/config.toml` with `target.x86_64-pc-windows-msvc.rustflags = ["-C", "target-feature=+crt-static"]`. CI matrix: `cargo fmt --check`, `clippy -D warnings`, `cargo test --workspace`, `cargo deny check`.
- **DoD.** All four CI jobs pass on a no-op PR. `cargo deny check` clean.
- **Criterion.** `cargo clippy --workspace --all-targets -- -D warnings` exits 0 and `gh workflow run ci.yml` finishes green on `main`.

### Task 2 — Engine core types & tracing scaffold

- **Owner.** Codex 5.5.
- **Crates / files.** [crates/engine/src/lib.rs](crates/engine/src/lib.rs), [crates/engine/src/error.rs](crates/engine/src/error.rs), [crates/engine/src/tracing.rs](crates/engine/src/tracing.rs), [crates/engine/Cargo.toml](crates/engine/Cargo.toml).
- **Depends on.** 1.
- **Hard rules.** Sets the scaffolding that lets later tasks comply with all of 1–10 (esp. observability for budget compliance).
- **What.** Add `thiserror`-based `EngineError` with variants (`Capture`, `Encode`, `Transport`, `Ipc`, `Protocol`). Public `init_tracing()` wires `tracing-subscriber` with `EnvFilter` and a JSON layer behind a `json` feature. Re-export `tracing::instrument`. Add `#[derive(CachePadded)]`-shaped wrapper `pub struct HotAtomic<T>` (newtype around `crossbeam_utils::CachePadded<T>`).
- **DoD.** `EngineError` is `Send + Sync + 'static`, no `unwrap()`, every public fn has rustdoc.
- **Criterion.** `cargo test -p engine -- --nocapture engine::tracing::tests::init_idempotent` green.

### Task 3 — Protocol: frame header proto + prost build

- **Owner.** Codex 5.5.
- **Crates / files.** [crates/protocol/Cargo.toml](crates/protocol/Cargo.toml), [crates/protocol/build.rs](crates/protocol/build.rs), [crates/protocol/proto/frame.proto](crates/protocol/proto/frame.proto), [crates/protocol/src/lib.rs](crates/protocol/src/lib.rs), [crates/protocol/src/frame.rs](crates/protocol/src/frame.rs).
- **Depends on.** 1, 2.
- **Hard rules.** §3 (no allocation in hot path → header `encode_to_slice` into preallocated `BytesMut`), §5 (sync API only — no async).
- **What.** Define `FrameHeader { stream_id u32, seq u64, pts_us u64, codec enum {H264, HEVC, AV1}, flags u32 (KEYFRAME, INTRA_REFRESH, LAST_SLICE), slice_idx u8, slice_count u8 }`. `build.rs` runs `prost-build`. Hand-written `FrameHeader::encode_to(&self, dst: &mut BytesMut) -> usize` and zero-copy `decode(src: &[u8]) -> Result<(Self, &[u8])>` (header + payload split) — do **not** use prost's allocating `Message::decode` on the hot path.
- **DoD.** Header is fixed-size 24 bytes, `repr(C)` POD; encode is alloc-free; decode is panic-free on truncated input.
- **Criterion.** `cargo test -p protocol frame::tests` green; `cargo +nightly miri test -p protocol frame::` green.

### Task 4 — Protocol: control RPC schemas

- **Owner.** Codex 5.5.
- **Crates / files.** [crates/protocol/proto/control.proto](crates/protocol/proto/control.proto), [crates/protocol/src/control.rs](crates/protocol/src/control.rs), [crates/protocol/proto/signaling.proto](crates/protocol/proto/signaling.proto).
- **Depends on.** 3.
- **Hard rules.** §2 (NACK & ref-frame invalidation messages must exist; no IDR-request opcode).
- **What.** Messages: `Hello { client_version, capabilities }`, `Bitrate { target_bps, ts }`, `Nack { stream_id, seq_ranges }`, `RefFrameInvalidate { last_good_seq }`, `InputEvent` (placeholder oneof), `Ping/Pong { tai64n }`. Signaling: `Offer { sdp_blob }`, `Answer { sdp_blob }`, `Candidate { ufrag, blob }`, `RoomCode { code: u32 }` (six digits → fits u32).
- **DoD.** No `idr_request` field anywhere. Backwards-compat field numbering reserved 1–15 for hot messages.
- **Criterion.** `cargo test -p protocol control::tests::roundtrip` green for all message types.

### Task 5 — Protocol: wire-format compatibility fixture

- **Owner.** Codex 5.5.
- **Crates / files.** [crates/protocol/tests/protocol-compat.rs](crates/protocol/tests/protocol-compat.rs), [crates/protocol/tests/fixtures/v0_1_0.bin](crates/protocol/tests/fixtures/v0_1_0.bin).
- **Depends on.** 3, 4.
- **Hard rules.** AGENT.md §"Things you must never do" — bw-incompat protocol edits.
- **What.** Generate a deterministic fixture of one of each control message + a frame header at version `v0.1.0`, commit the binary, write a test that decodes it with the current schema. Future PRs that break compat will break this test.
- **DoD.** Fixture committed; test reads it with `include_bytes!`.
- **Criterion.** `cargo test -p protocol --test protocol-compat` green.

---

## Phase 1B — Win32 host scaffold (tasks 6–7)

### Task 6 — host-service: Win32 service entry + SCM dispatch

- **Owner.** Sonnet 4.6 — service install/uninstall, SCM handshakes, and the SYSTEM-vs-session-0 trade-off are fiddly and need human sanity checks.
- **Crates / files.** [crates/host-service/src/main.rs](crates/host-service/src/main.rs), [crates/host-service/src/service.rs](crates/host-service/src/service.rs), [crates/host-service/src/install.rs](crates/host-service/src/install.rs), [crates/host-service/Cargo.toml](crates/host-service/Cargo.toml).
- **Depends on.** 2.
- **Hard rules.** §6 indirectly (service is what makes pen injection work later); AGENT.md `// SAFETY:` requirement on all `unsafe` Win32 calls.
- **What.** Use `windows-service 0.7`. CLI subcommands: `stream-host install`, `uninstall`, `run` (debug console), service entry registers via `service_dispatcher::start`. Service runs as `LocalSystem`, accepts `STOP | SHUTDOWN`. Empty `tokio::runtime::Builder::new_current_thread()` runtime placeholder for control plane.
- **DoD.** Installs as service `StreamHost`, starts, idles, stops cleanly. `// SAFETY:` on every `unsafe` block.
- **Criterion.** `sc.exe query StreamHost` shows `RUNNING` after `stream-host install && sc start StreamHost`; `cargo test -p host-service service::tests::lifecycle_console_mode` green.

### Task 7 — host-service: DPI awareness + MMCSS thread pinning utility

- **Owner.** Sonnet 4.6 — affects every later thread; needs review.
- **Crates / files.** [crates/engine/src/threading.rs](crates/engine/src/threading.rs), [crates/host-service/src/main.rs](crates/host-service/src/main.rs).
- **Depends on.** 2, 6.
- **Hard rules.** SKILL.md §"Capture stack" (PER_MONITOR_AWARE_V2), §"Performance budget".
- **What.** `engine::threading::set_process_dpi_awareness_v2()` calls `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)`. `pin_to_core(core_id: usize) -> Result<()>` via `SetThreadAffinityMask`. `register_mmcss(profile: MmcssProfile)` for `Pro Audio` / `Capture` / `Playback`. Used during host-service startup before any capture init.
- **DoD.** Manifest embedded at the host-service binary level (PER_MONITOR_AWARE_V2 fallback for old loaders). All `unsafe` annotated.
- **Criterion.** `cargo test -p engine threading::tests::pin_then_query` green; manual smoke: `stream-host run` logs `dpi=PER_MONITOR_AWARE_V2 mmcss=Pro Audio core=0` at INFO.

---

## Phase 1C — Capture (tasks 8–10)

### Task 8 — capture-dxgi: D3D11 device + shared NT-handle texture allocator

- **Owner.** Sonnet 4.6 — D3D11 device flags and NT-handle sharing across processes are a known footgun; review needed.
- **Crates / files.** [crates/engine/capture-dxgi/src/device.rs](crates/engine/capture-dxgi/src/device.rs), [crates/engine/capture-dxgi/src/shared_texture.rs](crates/engine/capture-dxgi/src/shared_texture.rs), [crates/engine/capture-dxgi/src/lib.rs](crates/engine/capture-dxgi/src/lib.rs).
- **Depends on.** 2, 7.
- **Hard rules.** §1 (zero CPU touch — texture is GPU-only), §3 (preallocate texture pool of 3, no runtime allocation).
- **What.** Pick adapter via `IDXGIFactory6::EnumAdapterByGpuPreference(HighPerformance)`. Create `ID3D11Device` with `BGRA_SUPPORT | VIDEO_SUPPORT`, feature level 11.1. `SharedTexturePool::new(width, height, fmt, count=3)` allocates `B8G8R8A8_UNORM` textures with `D3D11_RESOURCE_MISC_SHARED_NTHANDLE | D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX`, returns NT handles via `IDXGIResource1::CreateSharedHandle`.
- **DoD.** Pool `acquire()` returns a typed handle in O(1) with no allocation; releases via Drop.
- **Criterion.** `cargo test -p capture-dxgi shared_texture::tests::allocate_three_then_share` green on a Windows runner with a GPU; size matches `width*height*4` exactly.

### Task 9 — capture-dxgi: DuplicateOutput1 loop paced to SyncQPCTime

- **Owner.** Sonnet 4.6 — pacing math + DDA recovery on `ACCESS_LOST` is design-sensitive.
- **Crates / files.** [crates/engine/capture-dxgi/src/duplication.rs](crates/engine/capture-dxgi/src/duplication.rs), [crates/engine/capture-dxgi/src/pacer.rs](crates/engine/capture-dxgi/src/pacer.rs), [crates/engine/src/spsc.rs](crates/engine/src/spsc.rs).
- **Depends on.** 8.
- **Hard rules.** §1, §3, §4 (SPSC capacity 3 — not a queue), §5 (sync thread, not async), §8 (WGC fallback path must be reachable).
- **What.** `IDXGIOutput6::DuplicateOutput1` with format list `[B8G8R8A8_UNORM, R16G16B16A16_FLOAT]`. Capture thread: `CreateWaitableTimerExW(MANUAL_RESET | HIGH_RESOLUTION)`, period derived from `GetFrameStatistics::SyncQPCTime`. `AcquireNextFrame(0, …)` non-blocking. On success, copy the surface ref into a `SharedTexturePool` slot via `CopySubresourceRegion` (still GPU-side — no CPU copy), enqueue handle into `ringbuf::HeapRb<CapturedFrame>` capacity 3 (overwrite-oldest). On `DXGI_ERROR_ACCESS_LOST`, recreate duplication; on `WAIT_TIMEOUT`, push a "no-change" tick.
- **DoD.** Hot path is alloc-free after warm-up (verified with `dhat-rs` capture). On `ACCESS_LOST`, recovers within one frame interval.
- **Criterion.** `cargo test -p capture-dxgi duplication::tests::ten_seconds_no_drops` green (GPU runner): 600 frames @60fps, no drops > 1, p99 inter-arrival ≤ 17 ms.

### Task 10 — capture-wgc: WGC fallback adapter

- **Owner.** Sonnet 4.6 — hybrid-GPU detection and WGC's per-window vs per-monitor surface differences need judgment.
- **Crates / files.** [crates/engine/capture-wgc/src/lib.rs](crates/engine/capture-wgc/src/lib.rs), [crates/engine/capture-wgc/src/frame_pool.rs](crates/engine/capture-wgc/src/frame_pool.rs), [crates/engine/capture-dxgi/src/lib.rs](crates/engine/capture-dxgi/src/lib.rs) (selector).
- **Depends on.** 8, 9.
- **Hard rules.** §1, §3, §8.
- **What.** WGC via `Direct3D11CaptureFramePool::CreateFreeThreaded`. Settings: `IsBorderRequired=false`, `IsCursorCaptureEnabled=false`, `MinUpdateInterval=TimeSpan::from_micros(1)` (24H2 bug workaround). Hybrid-GPU detection: enumerate adapters; if more than one adapter and primary is integrated, force WGC. Common `trait Capture { fn next_frame(&mut self) -> Option<CapturedFrame>; }` implemented by both `DxgiCapture` and `WgcCapture`.
- **DoD.** Selector unit-tested with mocked adapter list.
- **Criterion.** `cargo test -p capture-wgc selector::tests::hybrid_picks_wgc` green; manual smoke on a hybrid-GPU laptop logs `capture=wgc reason=hybrid_gpu`.

---

## Phase 1D — Encoder (tasks 11–13)

### Task 11 — encode-nvenc-sys: bindgen FFI

- **Owner.** Codex 5.5 — bindgen + headers is mechanical.
- **Crates / files.** [crates/engine/encode-nvenc/sys/build.rs](crates/engine/encode-nvenc/sys/build.rs), [crates/engine/encode-nvenc/sys/wrapper.h](crates/engine/encode-nvenc/sys/wrapper.h), [crates/engine/encode-nvenc/sys/Cargo.toml](crates/engine/encode-nvenc/sys/Cargo.toml), [Cargo.toml](Cargo.toml) (workspace member).
- **Depends on.** 1.
- **Hard rules.** AGENT.md §"Coding conventions" — FFI lives in a `*-sys` crate with no logic.
- **What.** Vendor NVENC SDK 13 headers under `vendor/nvenc-sdk-13/` (gitignored binary; CI fetches). `bindgen 0.69` with `allowlist_function("NvEnc.*")`, `allowlist_type("NV_ENC_.*")`, dynamic loading via `libloading` for `nvEncodeAPI64.dll`. No safe wrappers in this crate — raw bindings only.
- **DoD.** `cargo doc -p encode-nvenc-sys` builds without warnings; no `unsafe` outside the generated module.
- **Criterion.** `cargo build -p encode-nvenc-sys` green; `nm` (or dumpbin) on the built rlib shows `NvEncodeAPICreateInstance` symbol resolution at runtime.

### Task 12 — encode-nvenc: safe wrapper + canonical P1 ULL config

- **Owner.** Codex 5.5 — config is *exactly* specified in SKILL.md §"Encoder stack"; do not deviate.
- **Crates / files.** [crates/engine/encode-nvenc/src/lib.rs](crates/engine/encode-nvenc/src/lib.rs), [crates/engine/encode-nvenc/src/config.rs](crates/engine/encode-nvenc/src/config.rs), [crates/engine/encode-nvenc/src/encoder.rs](crates/engine/encode-nvenc/src/encoder.rs).
- **Depends on.** 11.
- **Hard rules.** §2 (no B-frames, no IDR for loss recovery, infinite GOP + intra-refresh), §9 (CBR, single-frame VBV), §3.
- **What.** `NvencEncoder::new(NvencConfig)` applies *verbatim* the SKILL.md canonical config: `preset=P1_GUID`, `tuningInfo=ULTRA_LOW_LATENCY`, `enableEncodeAsync=0`, `gopLength=NVENC_INFINITE_GOPLENGTH`, `frameIntervalP=1`, `rateControlMode=CBR`, `vbvBufferSize=bitrate/fps`, `vbvInitialDelay=bitrate/fps`, `zeroReorderDelay=1`, `enableLookahead=0`, `intraRefreshPeriod=60`, `intraRefreshCnt=8`, `enableIntraRefresh=1`, `sliceMode=3`, `sliceModeData=4`, `maxNumRefFrames=8`, `ltrNumFrames=4`, `repeatSPSPPS=1`. Builder rejects any caller attempting to enable B-frames or set `idrPeriod < INFINITE`.
- **DoD.** All NVENC `NV_ENC_INITIALIZE_PARAMS` field values verified against SKILL.md table in `config::tests::matches_skill_md_canonical`.
- **Criterion.** `cargo test -p encode-nvenc config::tests::matches_skill_md_canonical` green; the test fails CI if any field drifts.

### Task 13 — encode-nvenc: zero-copy register + frame output to BytesMut pool

- **Owner.** Sonnet 4.6 — registering a shared NT-handle texture as an NVENC input resource has lifetime sharp edges.
- **Crates / files.** [crates/engine/encode-nvenc/src/register.rs](crates/engine/encode-nvenc/src/register.rs), [crates/engine/encode-nvenc/src/encoder.rs](crates/engine/encode-nvenc/src/encoder.rs), [crates/engine/src/bytes_pool.rs](crates/engine/src/bytes_pool.rs).
- **Depends on.** 9, 12.
- **Hard rules.** §1, §3, §4.
- **What.** `NV_ENC_REGISTER_RESOURCE` against the shared NT-handle from Task 8. Bitstream output buffer pool of 8 preallocated `BytesMut`. Encode loop: dequeue captured handle from SPSC, call `NvEncEncodePicture`, drain `NV_ENC_LOCK_BITSTREAM`, copy bitstream into a pooled `BytesMut`, enqueue `(FrameHeader, BytesMut)` onto net-tx SPSC. No allocation in steady state.
- **DoD.** `dhat-rs` shows zero heap allocations across 600 encode iterations after warm-up.
- **Criterion.** `cargo run -p latency-bench -- --resolution 1920x1080 --fps 144 --frames 600` reports encode p99 ≤ 3.0 ms on RTX 4070-class hardware (Task 24 implements the bench; until then, gate on `cargo test -p encode-nvenc encoder::tests::round_trip_600_frames` green).

---

## Phase 1E — Transport (tasks 14–16)

### Task 14 — transport-quic: Quinn server with self-signed cert

- **Owner.** Codex 5.5.
- **Crates / files.** [crates/engine/transport-quic/src/server.rs](crates/engine/transport-quic/src/server.rs), [crates/engine/transport-quic/src/tls.rs](crates/engine/transport-quic/src/tls.rs), [crates/engine/transport-quic/src/lib.rs](crates/engine/transport-quic/src/lib.rs).
- **Depends on.** 2, 4.
- **Hard rules.** §7 (Quinn TLS 1.3 + aws-lc-rs only).
- **What.** `quinn 0.11.9` with `quinn::default_runtime()` — but bound to a *single* tokio current-thread runtime owned by the net thread (not the engine's media threads — §5). `rustls 0.23` + `aws-lc-rs 1` provider. ALPN `stream/1`. TransportConfig: `max_concurrent_uni_streams(0)`, `max_concurrent_bidi_streams(2)`, `datagram_receive_buffer_size(8 * 1024 * 1024)`, `keep_alive_interval(Some(Duration::from_secs(15)))`. Self-signed dev cert via `rcgen`; in production, leaf cert pinned via TOFU.
- **DoD.** `accept().await` returns a `Connection` after one client connects; ALPN mismatch is rejected.
- **Criterion.** `cargo test -p transport-quic server::tests::loopback_handshake` green.

### Task 15 — transport-quic: client + datagram TX with PMTU-1200 slicing

- **Owner.** Codex 5.5.
- **Crates / files.** [crates/engine/transport-quic/src/client.rs](crates/engine/transport-quic/src/client.rs), [crates/engine/transport-quic/src/datagram_tx.rs](crates/engine/transport-quic/src/datagram_tx.rs), [crates/engine/transport-quic/src/slicer.rs](crates/engine/transport-quic/src/slicer.rs).
- **Depends on.** 3, 14.
- **Hard rules.** §7, RFC 9221 datagram framing, §3.
- **What.** Client connects with `endpoint.connect(addr, "stream.local")`. PMTU starts at 1200 (Quinn's default DPLPMTUD raises it). `slicer::slice(payload: &[u8], header: FrameHeader, mtu: usize) -> impl Iterator<Item = Datagram>` builds slices with `slice_idx`/`slice_count` set; payload split point chosen so each datagram ≤ `mtu - sizeof(FrameHeader)`. `Connection::send_datagram(Bytes)` per slice. Pacing: target 60–80% of frame interval, implemented as a token bucket on the net thread.
- **DoD.** Slicing is deterministic and lossless across `frames in {1KB, 50KB, 500KB, 5MB}` × `mtu in {1200, 1400}`.
- **Criterion.** `cargo test -p transport-quic slicer::tests::roundtrip_property` (proptest) green; loopback test sends 1000 frames of 200KB at 144 fps without drops.

### Task 16 — transport-quic: control bidi stream + bitrate feedback channel

- **Owner.** Codex 5.5.
- **Crates / files.** [crates/engine/transport-quic/src/control.rs](crates/engine/transport-quic/src/control.rs), [crates/engine/transport-quic/src/bitrate_signal.rs](crates/engine/transport-quic/src/bitrate_signal.rs).
- **Depends on.** 4, 15.
- **Hard rules.** §5 (control plane on tokio is fine; media stays sync), §7.
- **What.** Single bidi stream per connection carries length-delimited prost `ControlMessage`. `BitrateSignal { target_bps: AtomicU32 }` (CachePadded) read by encode thread once per frame; updated by net-rx every 50–100 ms from Quinn's `path_stats().congestion_window` mapped through a BBRv3-derived target (placeholder linear mapping for MVP — note `// FIXME(agent): replace with BBRv3 model in Phase 2`). `Nack` and `RefFrameInvalidate` sent over the bidi stream, parsed and surfaced via mpsc to the encode thread.
- **DoD.** Bitrate signal updates propagate within 100 ms; control messages don't compete with datagram queue.
- **Criterion.** `cargo test -p transport-quic control::tests::bitrate_propagates` green.

---

## Phase 1F — IPC (tasks 17–18)

### Task 17 — ipc: named-pipe JSON-RPC server scaffold

- **Owner.** Codex 5.5 — named-pipe + JSON-RPC is a well-trodden pattern.
- **Crates / files.** [crates/ipc/src/lib.rs](crates/ipc/src/lib.rs), [crates/ipc/src/server.rs](crates/ipc/src/server.rs), [crates/ipc/src/codec.rs](crates/ipc/src/codec.rs), [crates/ipc/Cargo.toml](crates/ipc/Cargo.toml).
- **Depends on.** 2, 4.
- **Hard rules.** §5 (IPC runs on the engine's tokio current-thread, *not* media threads).
- **What.** `tokio::net::windows::named_pipe::NamedPipeServer` at `\\.\pipe\stream.engine`. Length-prefixed JSON-RPC 2.0 frames. ACL: only `Authenticated Users` may connect (set via `SECURITY_ATTRIBUTES` with explicit DACL — `// SAFETY:` comment naming the principal). Method dispatch table; methods registered as `async fn(params: Value) -> Result<Value>`.
- **DoD.** Refuses connections with no auth token; one connection at a time (replace newer-wins).
- **Criterion.** `cargo test -p ipc server::tests::echo_method_roundtrip` green.

### Task 18 — ipc: Tauri client + Engine↔UI message contract

- **Owner.** Sonnet 4.6 — bridging into Tauri's command/event system needs design.
- **Crates / files.** [crates/ipc/src/client.rs](crates/ipc/src/client.rs), [app/src-tauri/src/ipc_bridge.rs](app/src-tauri/src/ipc_bridge.rs), [app/src-tauri/src/lib.rs](app/src-tauri/src/lib.rs), [app/src/lib/ipc.ts](app/src/lib/ipc.ts).
- **Depends on.** 17.
- **Hard rules.** None directly; respects §5.
- **What.** UI→Engine commands: `start_host()`, `connect_to_room({code: string})`, `disconnect()`. Engine→UI events: `connection_state {state: "Idle"|"Pairing"|"Connected"|"Failed"}`, `frame_stats {fps, bitrate_kbps, p99_ms}`, `room_code {code: string}`. Tauri side wraps the `IpcClient` in a `tauri::State<Arc<IpcClient>>` and forwards events via `Window::emit`.
- **DoD.** Schema documented in [crates/ipc/SCHEMA.md](crates/ipc/SCHEMA.md). TypeScript types mirror Rust (codegen via `ts-rs`).
- **Criterion.** `cargo test -p ipc client::tests::frame_stats_event_arrives` green; `pnpm --filter app typecheck` green.

---

## Phase 1G — Signaling (tasks 19–20)

### Task 19 — signaling: Worker scaffold + 6-digit code generator

- **Owner.** Codex 5.5.
- **Crates / files.** [signaling/src/worker.ts](signaling/src/worker.ts), [signaling/src/code.ts](signaling/src/code.ts), [signaling/wrangler.toml](signaling/wrangler.toml), [signaling/test/code.test.ts](signaling/test/code.test.ts).
- **Depends on.** 4.
- **Hard rules.** SKILL.md §"Signaling" — 6-digit codes, 5–10 min TTL, hibernating WS, DO authenticates neither peer.
- **What.** Worker routes: `POST /rooms` → mint code, `GET /rooms/:code/ws` → upgrade to WS. Code generator: 6 digits, exclude leading zero, exclude codes starting with `0` or `911`. Collision check against the DO namespace `ROOMS`. TTL 600 s via DO alarms.
- **DoD.** 1M codes generated in test show <0.1% collision rate before retry.
- **Criterion.** `pnpm --filter signaling test code.test.ts` green; `wrangler dev` answers `POST /rooms` with `{code: "######"}`.

### Task 20 — signaling: Durable Object hibernating WebSocket router

- **Owner.** Sonnet 4.6 — DO state machine + hibernation API needs review.
- **Crates / files.** [signaling/src/room.ts](signaling/src/room.ts), [signaling/test/room.test.ts](signaling/test/room.test.ts).
- **Depends on.** 19.
- **Hard rules.** SKILL.md §"Signaling" — hibernation so idle connections cost $0; auth happens P2P, not at DO.
- **What.** `Room` Durable Object holds at most 2 sockets (host, client). Use `state.acceptWebSocket(ws)` and `webSocketMessage(ws, msg)` for hibernation. Forwards `Offer`/`Answer`/`Candidate` between sockets unmodified. On second peer attaching, alarm at +60 s closes room if no traffic. No per-peer auth — relays whatever the peers send (binary frames pass through).
- **DoD.** Two Miniflare clients exchange offer/answer in <50 ms RTT.
- **Criterion.** `pnpm --filter signaling test room.test.ts -- --testNamePattern "two_peers_relay"` green.

---

## Phase 1H — UI (tasks 21–22)

### Task 21 — app: Tauri shell with Mica + transparent + decorations off

- **Owner.** Sonnet 4.6 — visual; needs human eyeball.
- **Crates / files.** [app/src-tauri/tauri.conf.json](app/src-tauri/tauri.conf.json), [app/src-tauri/src/window.rs](app/src-tauri/src/window.rs), [app/src/app/layout.tsx](app/src/app/layout.tsx), [app/src/app/globals.css](app/src/app/globals.css).
- **Depends on.** 1.
- **Hard rules.** SKILL.md §"UI" — `transparent: true, decorations: false`, Mica via `window-vibrancy`.
- **What.** `tauri.conf.json`: `transparent: true`, `decorations: false`, `width: 960`, `height: 600`, `minWidth: 720`, `minHeight: 480`. On window-create, call `apply_mica(&window, Some(true))`; on Win10, fall back to `apply_acrylic`. Custom title bar with `data-tauri-drag-region`, traffic lights right-aligned, Inter Variable + JetBrains Mono. OKLCH tokens defined in `globals.css`. Respect `prefers-reduced-motion`.
- **DoD.** Window opens with no chrome, blurred Mica background, title bar drag works.
- **Criterion.** `pnpm tauri dev` opens; manual visual check against [docs/ui-mock.png](docs/ui-mock.png) (placeholder ok in MVP — owner confirms by eye); `pnpm test:visual --grep "shell-renders"` (Playwright) green.

### Task 22 — app: Connect screen — code input + Connect button

- **Owner.** Sonnet 4.6 — visual + UX.
- **Crates / files.** [app/src/app/page.tsx](app/src/app/page.tsx), [app/src/components/CodeInput.tsx](app/src/components/CodeInput.tsx), [app/src/components/ConnectButton.tsx](app/src/components/ConnectButton.tsx), [app/src/lib/state.ts](app/src/lib/state.ts).
- **Depends on.** 18, 21.
- **Hard rules.** SKILL.md §"UI" — 220/140 ms asymmetric motion via Framer-Motion, shadcn/ui primitives.
- **What.** One screen: six-segment code input (auto-advance on digit, paste smart-fill), `ConnectButton` (primary, OKLCH accent), live status pill driven by the `connection_state` event from Task 18. On click, calls `invoke("connect_to_room", {code})`. No room creation flow yet (host side mints its own and shows it — Task 25).
- **DoD.** Storybook entry exists. Tab order is logical; `aria-live="polite"` on status pill.
- **Criterion.** `pnpm test:visual --grep "connect-screen"` (Playwright + Percy snapshot) green; manual smoke: typing `123456` enables the button.

---

## Phase 1I — Integration (tasks 23–25)

### Task 23 — tools/loopback: in-process host+client harness

- **Owner.** Codex 5.5.
- **Crates / files.** [tools/loopback/src/main.rs](tools/loopback/src/main.rs), [tools/loopback/src/transport_pair.rs](tools/loopback/src/transport_pair.rs), [tools/loopback/Cargo.toml](tools/loopback/Cargo.toml).
- **Depends on.** 13, 16.
- **Hard rules.** §5 (test harness can use a single tokio runtime, but media threads still sync).
- **What.** Spin up two `Engine` instances in one process: one host (capture+encode), one client (decode+verify-only). Replace Quinn `Endpoint` pair with an in-memory `TransportPair` adapter (Quinn-like trait wrapping `tokio::io::duplex`) so latency is deterministic and no UDP socket is opened. Used by Tasks 24 and 25.
- **DoD.** 60 s run shows zero frame drops, no allocations after warm-up (`dhat`).
- **Criterion.** `cargo run -p loopback -- --duration 30s --resolution 1920x1080 --fps 144` exits 0 with `dropped=0`.

### Task 24 — tools/latency-bench: capture+encode criterion harness

- **Owner.** Codex 5.5.
- **Crates / files.** [tools/latency-bench/src/main.rs](tools/latency-bench/src/main.rs), [tools/latency-bench/benches/pipeline.rs](tools/latency-bench/benches/pipeline.rs).
- **Depends on.** 13, 23.
- **Hard rules.** SKILL.md §"Performance budget".
- **What.** CLI flags `--resolution WxH --fps N --frames N --codec h264|hevc|av1`. Drives synthetic `SharedTexturePool` (RNG-filled textures via compute shader → encode) so it runs without a real desktop. Reports p50/p95/p99/p99.9 for capture-acquire, encode, slice, send-to-pair. `criterion` benchmark with throughput model. Emits JSON to stdout for CI regression baseline.
- **DoD.** Output schema documented; baseline JSON in [tools/latency-bench/baseline.json](tools/latency-bench/baseline.json) committed for CI diff.
- **Criterion.** `cargo run -p latency-bench --release -- --resolution 1920x1080 --fps 144 --frames 600` reports encode p99 ≤ 3.0 ms and total capture+encode p99 ≤ 4.0 ms on the dev box; CI fails if any p99 regresses >5% vs `baseline.json`.

### Task 25 — tests/e2e: single-machine end-to-end loopback

- **Owner.** Sonnet 4.6 — assembly, plus debugging the inevitable inter-component glue issues.
- **Crates / files.** [tests/e2e/Cargo.toml](tests/e2e/Cargo.toml), [tests/e2e/tests/single_machine.rs](tests/e2e/tests/single_machine.rs), [signaling/test/integration.test.ts](signaling/test/integration.test.ts).
- **Depends on.** 5, 7, 10, 13, 16, 18, 20, 22, 23, 24.
- **Hard rules.** All ten — this is the integration check.
- **What.** Test driver: (a) starts the Worker via `wrangler dev` on port 8787, (b) launches `host-service` in console mode, (c) launches the Tauri shell in headless mode (`tauri::test::mock_runtime`), (d) UI calls `start_host()` → engine asks Worker for a room code → engine starts QUIC listener on a loopback socket, (e) a second engine instance plays the role of "client", calls `connect_to_room(code)`, completes signaling, opens QUIC, (f) host engine loops captured frames into the encoder, (g) client engine receives ≥120 frames over 1 s and reconstructs the FrameHeader sequence with no gaps. Asserts no IDR frames in the bitstream (§2) and that bitrate adaptation message moved at least once (§"Network stack").
- **DoD.** Test runs in CI on a Windows 11 self-hosted runner with a GPU; runtime ≤ 90 s.
- **Criterion.** `cargo test -p e2e --test single_machine -- --nocapture` exits 0 with logged glass-to-glass median ≤ 25 ms over the loopback adapter.

---

## Topological summary (each task depends only on earlier ones)

```
1
├── 2
│   ├── 3 ── 4 ── 5
│   ├── 6 ── 7 ── 8 ── 9 ── 10
│   ├── 11 ── 12 ── 13
│   ├── 14 ── 15 ── 16
│   ├── 17 ── 18
│   ├── 19 ── 20
│   └── 21 ── 22
├── 23 (needs 13, 16)
├── 24 (needs 13, 23)
└── 25 (needs 5, 7, 10, 13, 16, 18, 20, 22, 23, 24)
```

## Owner mix

- **Codex 5.5 (autonomous): 13 tasks** — 1, 2, 3, 4, 5, 11, 12, 14, 15, 16, 17, 19, 23, 24.
- **Sonnet 4.6 (interactive): 12 tasks** — 6, 7, 8, 9, 10, 13, 18, 20, 21, 22, 25.

## Phase 1 exit criteria

All 25 tasks merged. `cargo test --workspace` green. `cargo run -p latency-bench --release -- --resolution 1920x1080 --fps 144` reports encode p99 ≤ 3 ms. `cargo test -p e2e --test single_machine` green. Tauri shell launches on Win11 with Mica, accepts a six-digit code, drives a loopback session that hits glass-to-glass median ≤ 25 ms and never emits an IDR frame.
