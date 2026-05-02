//! Wire protocol types for Stream control messages and media frame headers.

pub mod control;
pub mod frame;

/// Prost-generated protobuf modules.
pub mod generated {
    /// Generated frame schema types.
    pub mod frame {
        include!(concat!(env!("OUT_DIR"), "/stream.protocol.frame.rs"));
    }

    /// Generated control RPC schema types.
    pub mod control {
        include!(concat!(env!("OUT_DIR"), "/stream.protocol.control.rs"));
    }

    /// Generated signaling schema types.
    pub mod signaling {
        include!(concat!(env!("OUT_DIR"), "/stream.protocol.signaling.rs"));
    }
}
