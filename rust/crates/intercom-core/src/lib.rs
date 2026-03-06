//! Intercom Core - Main facade for the intercom system.
//!
//! This crate provides the high-level API for building intercom applications,
//! combining audio I/O, codec, networking, and session management.

pub mod client;
pub mod server;
pub mod config;
pub mod events;

pub use client::IntercomClient;
pub use server::IntercomServer;
pub use config::{ClientConfig, ServerConfig};
pub use events::{IntercomEvent, EventHandler};

// Re-export commonly used types from other crates
pub use intercom_protocol::{
    ChannelId, ChannelInfo, RoomId, UserId, UserInfo,
    audio_params, PROTOCOL_VERSION,
};
pub use intercom_audio::{AudioDevice, DeviceType, AudioBuffer};
pub use intercom_transport::ConnectionState;

use thiserror::Error;

/// Core errors.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("Audio error: {0}")]
    Audio(#[from] intercom_audio::AudioError),

    #[error("Codec error: {0}")]
    Codec(#[from] intercom_codec::CodecError),

    #[error("Transport error: {0}")]
    Transport(#[from] intercom_transport::TransportError),

    #[error("Signaling error: {0}")]
    Signaling(#[from] intercom_signaling::SignalingError),

    #[error("Session error: {0}")]
    Session(#[from] intercom_session::SessionError),

    #[error("Crypto error: {0}")]
    Crypto(#[from] intercom_crypto::CryptoError),

    #[error("Not connected")]
    NotConnected,

    #[error("Already connected")]
    AlreadyConnected,

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Operation timeout")]
    Timeout,
}

/// Initialize logging for the intercom system.
pub fn init_logging() {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(filter)
        .init();
}

/// Version information.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!version().is_empty());
    }
}
