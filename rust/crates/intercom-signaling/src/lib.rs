//! Firebase signaling client for WebRTC connection establishment.
//!
//! This module provides signaling functionality using Firebase Realtime Database
//! for exchanging SDP offers/answers and ICE candidates.

pub mod firebase;

pub use firebase::{FirebaseConfig, FirebaseSignaling};

use thiserror::Error;

/// Signaling errors.
#[derive(Debug, Error)]
pub enum SignalingError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Firebase error: {0}")]
    Firebase(String),

    #[error("Connection timeout")]
    Timeout,

    #[error("Peer not found: {0}")]
    PeerNotFound(String),

    #[error("Room not found: {0}")]
    RoomNotFound(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

/// Signaling message types.
#[derive(Debug, Clone)]
pub enum SignalMessage {
    /// SDP offer.
    Offer { sdp: String },
    /// SDP answer.
    Answer { sdp: String },
    /// ICE candidate.
    IceCandidate {
        candidate: String,
        sdp_mid: Option<String>,
        sdp_mline_index: Option<u16>,
    },
}
