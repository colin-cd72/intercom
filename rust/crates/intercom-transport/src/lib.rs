//! WebRTC transport layer for the intercom system.
//!
//! Provides peer-to-peer audio transmission using WebRTC data channels.

pub mod webrtc;

pub use self::webrtc::{IceCandidate, PeerConnection, TransportConfig};

use thiserror::Error;

/// Transport errors.
#[derive(Debug, Error)]
pub enum TransportError {
    #[error("WebRTC error: {0}")]
    WebRTC(String),

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("ICE gathering failed")]
    IceGatheringFailed,

    #[error("Signaling error: {0}")]
    SignalingError(String),

    #[error("Channel closed")]
    ChannelClosed,

    #[error("Timeout")]
    Timeout,

    #[error("Invalid SDP: {0}")]
    InvalidSdp(String),

    #[error("Peer not connected")]
    NotConnected,
}

/// Public STUN servers for NAT traversal.
pub const STUN_SERVERS: &[&str] = &[
    "stun:stun.l.google.com:19302",
    "stun:stun1.l.google.com:19302",
    "stun:stun2.l.google.com:19302",
    "stun:stun.cloudflare.com:3478",
];

/// Connection state for peer connections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    New,
    Connecting,
    Connected,
    Disconnected,
    Failed,
    Closed,
}

impl std::fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionState::New => write!(f, "New"),
            ConnectionState::Connecting => write!(f, "Connecting"),
            ConnectionState::Connected => write!(f, "Connected"),
            ConnectionState::Disconnected => write!(f, "Disconnected"),
            ConnectionState::Failed => write!(f, "Failed"),
            ConnectionState::Closed => write!(f, "Closed"),
        }
    }
}
