# SKILL.md — Building "Stream" (Windows-only premium P2P remote desktop)

## Project mission
Beat Parsec, Moonlight, AnyDesk, and RustDesk on latency, quality, and UX. Target: 1–3 ms LAN, 10–25 ms WAN glass-to-glass. Free to operate ($0/mo infrastructure) plus optional cheap signing ($9.99/mo Azure Trusted Signing). Windows 10 22H2+ and Windows 11 23H2/24H2/25H2.

## Architecture in one diagram
HOST PROCESS (Rust, no UI, runs as SYSTEM service for UAC capture):
- T1 capture (core 0, MMCSS Pro Audio, TIME_CRITICAL): DXGI Desktop Duplication primary, WGC fallback. Output: shared NT-handle ID3D11Texture2D + dirty rects.
- T2 encode (core 1): NVENC P1 ULL → AMF VCN5 → QSV Battlemage AV1 SCC → x264 ultrafast+zerolatency. Zero-copy from T1.
- T3 net-tx (core 2): Quinn QUIC datagrams + control bidi stream. Reed-Solomon FEC.
- T4 net-rx (core 3): Quinn ingress, NACK and invalidation upcalls.
- T5 decode (core 4): D3D11VA / DXVA2.
- T6 render (core 5): D3D11 flip-model swapchain with ALLOW_TEARING for borderless.
- Tokio current-thread (core 6): control plane, IPC server, signaling, updater.

UI PROCESS (Tauri 2, WebView2):
- Next.js + React 19 + Tailwind 4 + shadcn/ui + Radix + Framer-Motion.
- IPC to engine via Windows named pipes (JSON-RPC).
- Mica backdrop via `window-vibrancy`.

## Hard rules (never violate)
1. Never CPU-touch a video frame. Capture → encode is GPU shared texture only.
2. Never B-frames. Never IDR keyframes for loss recovery (use NVENC reference-frame invalidation). Always infinite GOP + intra-refresh.
3. Never allocate in the hot path after warm-up. Use `bumpalo`, preallocated `BytesMut` pools, `ringbuf` SPSC.
4. Never block the encode thread. SPSC handoff capacity is 3, not a queue.
5. Never run media pipeline on async. Tokio is for control plane only.
6. Never use `MOUSEEVENTF_PEN`. Use `InjectSyntheticPointerInput`.
7. Never roll your own crypto. Quinn's TLS 1.3 + `aws-lc-rs`. Noise via `snow`.
8. Never trust DDA across hybrid GPUs. Always WGC fallback.
9. Never use VBR for real-time CBR. Single-frame VBV mandatory.
10. Never forward keyboard with virtual keys. Scan codes only.

## Capture stack
DDA via `IDXGIOutputDuplication::DuplicateOutput1` with format list `{B8G8R8A8_UNORM, R16G16B16A16_FLOAT}`. Process must be PER_MONITOR_AWARE_V2. Per-output thread, `AcquireNextFrame(0, ...)` paced by waitable timer locked to `IDXGIOutput::GetFrameStatistics::SyncQPCTime`. WGC fallback: `Direct3D11CaptureFramePool::CreateFreeThreaded`, `IsBorderRequired=false`, `IsCursorCaptureEnabled=false`, `MinUpdateInterval=1us` (24H2 bug workaround). Cursor via DDA pointer info or `GetCursorInfo`+`CopyImage` separately, transmitted out-of-band.

## Encoder stack
NVENC SDK 13 canonical config (don't deviate):
preset=P1_GUID, tuningInfo=ULTRA_LOW_LATENCY, enableEncodeAsync=0, gopLength=NVENC_INFINITE_GOPLENGTH, frameIntervalP=1, rateControlMode=CBR, vbvBufferSize=bitrate/fps, vbvInitialDelay=bitrate/fps, zeroReorderDelay=1, enableLookahead=0, intraRefreshPeriod=60, intraRefreshCnt=8, enableIntraRefresh=1, sliceMode=3, sliceModeData=4, maxNumRefFrames=8, ltrNumFrames=4, repeatSPSPPS=1.

Codec ranking for screen content: AV1 with SCC tools (Battlemage QSV) > HEVC 4:4:4 > HEVC 4:2:0 > H.264 4:4:4 > H.264 4:2:0 > VP9 (avoid). 4:4:4 mandatory in High Quality and Lossless modes.

Loss recovery: NVENC reference invalidation primary, Reed-Solomon FEC across slices secondary, NACK only when RTT < 0.5 frame budget. Never force IDR.

## Network stack
Quinn 0.11.9 QUIC. Video over RFC 9221 unreliable datagrams sliced to PMTU-1200. Control/clipboard/file over reliable bidirectional streams. TLS 1.3 + AES-128-GCM (ChaCha20 fallback via runtime CPU detection). BBRv3 congestion control feeding bitrate target to encoder every 50–100 ms. PMTUD via Quinn's built-in (RFC 8899). Pacing: 60–80% of frame interval to soften WiFi.

## NAT traversal
ICE-style parallel attempts: IPv6 direct → IPv4 + STUN (cloudflare.com first, l.google.com fallback) → port prediction for symmetric NAT → TCP/443 → TURN UDP → TURN TCP/443. Self-hosted coturn on Oracle Cloud Always Free (4 ARM cores, 24 GB RAM, 10 TB egress/mo, free forever). Cloudflare Realtime TURN as anycast fallback (free ≤1 TB/mo, then $0.05/GB).

## Signaling
Cloudflare Workers + Durable Objects. 6-digit room codes, 5–10 min TTL, hibernating WebSockets so idle connections cost $0. DO authenticates neither peer; auth happens P2P via SPAKE2+ over data channel.

## Auth
First pair: SPAKE2+ (RFC 9383) on 6-digit PIN displayed on host. Exchange Ed25519 device keys, pin TOFU. Per-session: Noise_IK_25519_AESGCM_BLAKE2s via `snow` crate. Force key updates every 30 min or 2^28 packets. TAI64N timestamp first message for replay protection.

## Input
SendInput baseline: keyboard scan codes (KEYEVENTF_SCANCODE on both down and up), mouse absolute (VIRTUALDESK + ABSOLUTE) for desktop, mouse MOUSEEVENTF_MOVE relative for raw-input games. Client uses RegisterRawInputDevices + GetRawInputBuffer for 8000 Hz mice in a message-only window with RIDEV_NOLEGACY. Gamepads via ViGEmBus today (plan VirtualPad migration). Pen/touch via InjectSyntheticPointerInput. Interception driver opt-in only, never on Vanguard systems. UAC-elevated targets require SYSTEM service + uiAccess manifest.

## Audio
WASAPI shared loopback, AUDCLNT_STREAMFLAGS_LOOPBACK | EVENTCALLBACK, 10 ms period. Per-process loopback (Win10 20348+) via PROCESS_LOOPBACK + PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE for echo cancellation. Opus 10 ms frames, OPUS_APPLICATION_RESTRICTED_LOWDELAY, INBAND_FEC=1, PACKET_LOSS_PERC=5. Mic-from-client via Parsec-style virtual capture device.

## UI
Tauri 2 + Next.js + Tailwind + Framer-Motion + shadcn/ui + Radix. Mica via window-vibrancy `apply_mica` (Win11) or `apply_acrylic` (Win10 fallback). `transparent: true, decorations: false` in tauri.conf.json. Custom title bar with data-tauri-drag-region and Ctrl+K command palette. Inter Variable + JetBrains Mono. OKLCH color tokens. 220/140 ms asymmetric motion. Respect prefers-reduced-motion. Virtualize lists with @tanstack/react-virtual.

Quality/latency slider: Fastest (P1, 4:2:0, 4 slices), Balanced (P3, 4:2:0, 2 slices), High Quality (P5, 4:4:4), Lossless (NVENC HEVC lossless, LAN only).

## Installer
Tauri NSIS primary, WiX MSI optional for enterprise. Tauri updater plugin with Ed25519 manifests. inner exe with RFC 3161 timestamp at timestamp.acs.microsoft.com. Public Trust profile.

## Risk register (always check)
HDCP returns black, document and detect. Secure desktop needs SYSTEM service. Vanguard incompatible — document. ViGEmBus archived — abstract behind trait. Hybrid GPU laptops need WGC fallback. Win10 22H2 EOL Oct 2025 — minimum-supported with warning.

## Performance budget (must hit)
LAN 1080p120: vsync wait 0–8.3 ms, capture 0.3–0.8 ms, encode 1–3 ms, send 0.2 ms, propagation 0.5–1 ms, decode 3–8 ms, scanout 0–8.3 ms. Total 5–25 ms.

## Forbidden libraries
Electron (bloat). libwebrtc / webrtc-rs (Arc/callback hell, jitter buffer floor). C# even NativeAOT (GC). Flutter desktop (uncanny on Windows). VBR for real-time. Software codecs in production hot path (only as fallback).

## Required crate versions (May 2026)
windows 0.61, quinn 0.11.9, str0m latest, rustls 0.23 + aws-lc-rs 1, ringbuf 0.4.9, crossbeam-utils 0.8, bumpalo 3, tokio 1, snow latest, ed25519-dalek 2, x25519-dalek 2, reed-solomon-erasure latest, tauri 2.x, window-vibrancy 0.6, bytes 1, tracing 0.1, bindgen 0.69 (build-dep), nvidia-video-codec-sdk latest.

## When asked to implement a feature
Always start by stating which thread/process it lives in, what data flows in and out via which ring buffer, and which hot-path constraints apply. Show the canonical NVENC config or DDA call before writing code. Quote the exact `windows-rs` API names. Prefer measurement (`tracing`, `dhat-rs`) over speculation. When code conflicts with a hard rule, refuse and surface the rule.