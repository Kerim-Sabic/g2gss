use thiserror::Error;

/// Error type shared by the engine orchestration boundary.
#[derive(Debug, Error)]
pub enum EngineError {
    /// Capture subsystem failure.
    #[error("capture error: {0}")]
    Capture(String),
    /// Encoder subsystem failure.
    #[error("encode error: {0}")]
    Encode(String),
    /// Transport subsystem failure.
    #[error("transport error: {0}")]
    Transport(String),
    /// Engine/UI IPC failure.
    #[error("ipc error: {0}")]
    Ipc(String),
    /// Wire protocol failure.
    #[error("protocol error: {0}")]
    Protocol(String),
}
