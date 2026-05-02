## Summary

-

## Process, Thread, And Data Flow

- Process:
- Thread:
- Ring buffer:
- Control message:

## Wire Protocol

- [ ] No wire-format change.
- [ ] Wire-format change is backwards compatible and covered by `tests/protocol-compat.rs`.
- [ ] Human approval recorded for any backwards-incompatible change.

## Validation

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `cargo deny check`
- [ ] `cargo run -p latency-bench -- --resolution 1920x1080 --fps 144`
- [ ] `cargo +nightly miri test -p protocol`
- [ ] `pnpm test`
- [ ] UI visual tests, if UI changed.
- [ ] Encoder conformance, if encoder changed.

## Risk Register

- [ ] HDCP black-frame handling considered.
- [ ] Secure desktop / SYSTEM service impact considered.
- [ ] Vanguard incompatibility considered.
- [ ] Hybrid GPU WGC fallback considered.
- [ ] Win10 22H2 support warning considered.

## Notes

-
