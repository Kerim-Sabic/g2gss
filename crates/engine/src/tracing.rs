use std::sync::{Once, OnceLock};

use thiserror::Error;
use tracing_subscriber::EnvFilter;

static INIT: Once = Once::new();
static INIT_RESULT: OnceLock<Result<(), InitTracingError>> = OnceLock::new();

/// Error returned when global tracing subscriber installation fails.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum InitTracingError {
    /// Another global subscriber was installed before the engine tracing subscriber.
    #[error("global tracing subscriber is already installed")]
    GlobalSubscriberAlreadyInstalled,
}

/// Initializes process-wide tracing for engine and control-plane code.
///
/// The function is idempotent for this crate: repeated calls return the first
/// initialization result without trying to replace the global subscriber.
pub fn init_tracing() -> Result<(), InitTracingError> {
    INIT.call_once(|| {
        let _ = INIT_RESULT.set(install_tracing());
    });

    match INIT_RESULT.get() {
        Some(result) => *result,
        None => Ok(()),
    }
}

#[cfg(not(feature = "json"))]
fn install_tracing() -> Result<(), InitTracingError> {
    tracing_subscriber::fmt()
        .with_env_filter(env_filter())
        .try_init()
        .map_err(|_| InitTracingError::GlobalSubscriberAlreadyInstalled)
}

#[cfg(feature = "json")]
fn install_tracing() -> Result<(), InitTracingError> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(env_filter())
        .try_init()
        .map_err(|_| InitTracingError::GlobalSubscriberAlreadyInstalled)
}

fn env_filter() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
}
