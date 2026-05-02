# Stream

Windows-only premium P2P remote desktop. See [SKILL.md](SKILL.md) for the architecture rules and [AGENT.md](AGENT.md) for the operating manual.

## Development Commands

```powershell
cargo build --release
cargo test --workspace
cargo run -p latency-bench -- --resolution 1920x1080 --fps 144
pnpm tauri dev
pnpm tauri build --target x86_64-pc-windows-msvc
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo deny check
cargo +nightly miri test -p protocol
```

Run the Tauri commands from `app/`.
