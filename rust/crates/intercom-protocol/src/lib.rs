//! Protocol message types for the intercom system.
//!
//! This crate defines all message types used for communication between
//! clients and servers in the intercom system.

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod messages;

pub use messages::*;

/// Protocol version for compatibility checking.
pub const PROTOCOL_VERSION: u32 = 1;

/// Audio parameters used throughout the system.
pub mod audio_params {
    /// Sample rate in Hz.
    pub const SAMPLE_RATE: u32 = 48000;
    /// Number of audio channels (mono).
    pub const CHANNELS: u16 = 1;
    /// Frame size in samples (10ms at 48kHz).
    pub const FRAME_SIZE: usize = 480;
    /// Frame duration in milliseconds.
    pub const FRAME_DURATION_MS: u32 = 10;
    /// Target bitrate for Opus encoder in bits per second.
    pub const OPUS_BITRATE: i32 = 48000;
}

/// Errors that can occur during protocol operations.
#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Invalid message type: {0}")]
    InvalidMessageType(u8),

    #[error("Version mismatch: expected {expected}, got {actual}")]
    VersionMismatch { expected: u32, actual: u32 },

    #[error("Invalid payload size: {0}")]
    InvalidPayloadSize(usize),
}

/// Unique identifier for a user.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(pub String);

impl UserId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a channel.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChannelId(pub String);

impl ChannelId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ChannelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a room/session.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RoomId(pub String);

impl RoomId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RoomId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// User information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: UserId,
    pub display_name: String,
    pub is_talking: bool,
    pub subscribed_channels: Vec<ChannelId>,
    pub talk_channel: Option<ChannelId>,
}

impl UserInfo {
    pub fn new(id: UserId, display_name: impl Into<String>) -> Self {
        Self {
            id,
            display_name: display_name.into(),
            is_talking: false,
            subscribed_channels: Vec::new(),
            talk_channel: None,
        }
    }
}

/// Channel information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelInfo {
    pub id: ChannelId,
    pub name: String,
    pub description: Option<String>,
    pub members: Vec<UserId>,
}

impl ChannelInfo {
    pub fn new(id: ChannelId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            description: None,
            members: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_id() {
        let id = UserId::new("user123");
        assert_eq!(id.as_str(), "user123");
        assert_eq!(format!("{}", id), "user123");
    }

    #[test]
    fn test_channel_id() {
        let id = ChannelId::new("channel1");
        assert_eq!(id.as_str(), "channel1");
    }

    #[test]
    fn test_user_info_serialization() {
        let user = UserInfo::new(UserId::new("user1"), "Test User");
        let json = serde_json::to_string(&user).unwrap();
        let deserialized: UserInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, user.id);
        assert_eq!(deserialized.display_name, user.display_name);
    }
}
