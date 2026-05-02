//! Core engine primitives shared by capture, encode, transport, and IPC crates.

pub mod error;
pub mod tracing;

use crossbeam_utils::CachePadded;

pub use ::tracing::instrument;
pub use error::EngineError;
pub use tracing::{InitTracingError, init_tracing};

/// Cache-line padded wrapper for atomics and other hot shared values.
///
/// Use this for state touched by more than one engine thread so false sharing is
/// avoided at the type boundary.
#[derive(Clone, Debug, Default)]
#[repr(transparent)]
pub struct HotAtomic<T>(CachePadded<T>);

impl<T> HotAtomic<T> {
    /// Creates a cache-line padded value.
    pub fn new(value: T) -> Self {
        Self(CachePadded::new(value))
    }

    /// Returns a shared reference to the padded value.
    pub fn get(&self) -> &T {
        &self.0
    }

    /// Returns an exclusive reference to the padded value.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T> From<T> for HotAtomic<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::{EngineError, HotAtomic};

    fn assert_send_sync_static<T: Send + Sync + 'static>() {}

    #[test]
    fn engine_error_is_thread_safe_static() {
        assert_send_sync_static::<EngineError>();
    }

    #[test]
    fn hot_atomic_wraps_atomic_state() {
        let value = HotAtomic::new(AtomicU32::new(7));

        assert_eq!(value.get().load(Ordering::Relaxed), 7);
    }
}

#[cfg(test)]
mod engine {
    pub mod tracing {
        pub mod tests {
            #[test]
            fn init_idempotent() {
                assert!(crate::init_tracing().is_ok());
                assert!(crate::init_tracing().is_ok());
            }
        }
    }
}
