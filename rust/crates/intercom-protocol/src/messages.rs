//! Protocol message definitions.

use serde::{Deserialize, Serialize};

use crate::{ChannelId, ChannelInfo, RoomId, UserId, UserInfo};

/// Message types for client-to-server communication.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum ClientMessage {
    /// Request to join a room.
    JoinRoom {
        room_id: RoomId,
        user_info: UserInfo,
        password: Option<String>,
    },

    /// Request to leave the current room.
    LeaveRoom,

    /// Subscribe to a channel for listening.
    SubscribeChannel { channel_id: ChannelId },

    /// Unsubscribe from a channel.
    UnsubscribeChannel { channel_id: ChannelId },

    /// Start talking on a channel.
    StartTalk { channel_id: ChannelId },

    /// Stop talking.
    StopTalk,

    /// Audio data packet.
    AudioData {
        sequence: u32,
        timestamp: u64,
        channel_id: ChannelId,
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
    },

    /// Ping for connection keep-alive.
    Ping { timestamp: u64 },

    /// Update user display name.
    UpdateDisplayName { name: String },
}

/// Message types for server-to-client communication.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum ServerMessage {
    /// Welcome message after successful join.
    Welcome {
        protocol_version: u32,
        user_id: UserId,
        room_info: RoomInfo,
    },

    /// Error response.
    Error { code: ErrorCode, message: String },

    /// Room state update.
    RoomUpdate { room_info: RoomInfo },

    /// User joined the room.
    UserJoined { user: UserInfo },

    /// User left the room.
    UserLeft { user_id: UserId },

    /// User started talking.
    UserTalkStart {
        user_id: UserId,
        channel_id: ChannelId,
    },

    /// User stopped talking.
    UserTalkStop { user_id: UserId },

    /// Mixed audio data for playback.
    AudioData {
        sequence: u32,
        timestamp: u64,
        channel_id: ChannelId,
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
        speakers: Vec<UserId>,
    },

    /// Pong response to ping.
    Pong { timestamp: u64, server_time: u64 },

    /// Channel created.
    ChannelCreated { channel: ChannelInfo },

    /// Channel deleted.
    ChannelDeleted { channel_id: ChannelId },
}

/// Room information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomInfo {
    pub id: RoomId,
    pub name: String,
    pub channels: Vec<ChannelInfo>,
    pub users: Vec<UserInfo>,
}

impl RoomInfo {
    pub fn new(id: RoomId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            channels: Vec::new(),
            users: Vec::new(),
        }
    }
}

/// Error codes for protocol errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum ErrorCode {
    /// Unknown error.
    Unknown = 0,
    /// Invalid password.
    InvalidPassword = 1,
    /// Room not found.
    RoomNotFound = 2,
    /// Channel not found.
    ChannelNotFound = 3,
    /// User not found.
    UserNotFound = 4,
    /// Already in room.
    AlreadyInRoom = 5,
    /// Not in room.
    NotInRoom = 6,
    /// Permission denied.
    PermissionDenied = 7,
    /// Rate limited.
    RateLimited = 8,
    /// Protocol version mismatch.
    VersionMismatch = 9,
    /// Room full.
    RoomFull = 10,
    /// Invalid request.
    InvalidRequest = 11,
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorCode::Unknown => write!(f, "Unknown error"),
            ErrorCode::InvalidPassword => write!(f, "Invalid password"),
            ErrorCode::RoomNotFound => write!(f, "Room not found"),
            ErrorCode::ChannelNotFound => write!(f, "Channel not found"),
            ErrorCode::UserNotFound => write!(f, "User not found"),
            ErrorCode::AlreadyInRoom => write!(f, "Already in room"),
            ErrorCode::NotInRoom => write!(f, "Not in room"),
            ErrorCode::PermissionDenied => write!(f, "Permission denied"),
            ErrorCode::RateLimited => write!(f, "Rate limited"),
            ErrorCode::VersionMismatch => write!(f, "Protocol version mismatch"),
            ErrorCode::RoomFull => write!(f, "Room full"),
            ErrorCode::InvalidRequest => write!(f, "Invalid request"),
        }
    }
}

/// Signaling messages for WebRTC connection establishment.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum SignalingMessage {
    /// SDP offer from initiator.
    Offer {
        from: UserId,
        to: UserId,
        sdp: String,
    },

    /// SDP answer from responder.
    Answer {
        from: UserId,
        to: UserId,
        sdp: String,
    },

    /// ICE candidate.
    IceCandidate {
        from: UserId,
        to: UserId,
        candidate: String,
        sdp_mid: Option<String>,
        sdp_mline_index: Option<u16>,
    },
}

/// Audio packet for transmission over the wire.
#[derive(Debug, Clone)]
pub struct AudioPacket {
    /// Sequence number for ordering.
    pub sequence: u32,
    /// Timestamp in samples.
    pub timestamp: u64,
    /// Source channel.
    pub channel_id: ChannelId,
    /// Source user.
    pub user_id: UserId,
    /// Opus-encoded audio data.
    pub data: Vec<u8>,
    /// Whether this packet contains voice (VAD).
    pub contains_voice: bool,
}

impl AudioPacket {
    /// Create a new audio packet.
    pub fn new(
        sequence: u32,
        timestamp: u64,
        channel_id: ChannelId,
        user_id: UserId,
        data: Vec<u8>,
    ) -> Self {
        Self {
            sequence,
            timestamp,
            channel_id,
            user_id,
            data,
            contains_voice: true,
        }
    }

    /// Serialize the packet for transmission.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(32 + self.data.len());

        // Header: sequence (4) + timestamp (8) + flags (1) + channel_len (2) + user_len (2) + data_len (2)
        buf.extend_from_slice(&self.sequence.to_le_bytes());
        buf.extend_from_slice(&self.timestamp.to_le_bytes());
        buf.push(if self.contains_voice { 1 } else { 0 });

        let channel_bytes = self.channel_id.0.as_bytes();
        buf.extend_from_slice(&(channel_bytes.len() as u16).to_le_bytes());
        buf.extend_from_slice(channel_bytes);

        let user_bytes = self.user_id.0.as_bytes();
        buf.extend_from_slice(&(user_bytes.len() as u16).to_le_bytes());
        buf.extend_from_slice(user_bytes);

        buf.extend_from_slice(&(self.data.len() as u16).to_le_bytes());
        buf.extend_from_slice(&self.data);

        buf
    }

    /// Deserialize a packet from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, crate::ProtocolError> {
        if data.len() < 15 {
            return Err(crate::ProtocolError::InvalidPayloadSize(data.len()));
        }

        let mut offset = 0;

        let sequence = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
        offset += 4;

        let timestamp = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
        offset += 8;

        let contains_voice = data[offset] != 0;
        offset += 1;

        let channel_len = u16::from_le_bytes(data[offset..offset + 2].try_into().unwrap()) as usize;
        offset += 2;

        if data.len() < offset + channel_len {
            return Err(crate::ProtocolError::InvalidPayloadSize(data.len()));
        }
        let channel_id = ChannelId::new(
            String::from_utf8_lossy(&data[offset..offset + channel_len]).to_string(),
        );
        offset += channel_len;

        let user_len = u16::from_le_bytes(data[offset..offset + 2].try_into().unwrap()) as usize;
        offset += 2;

        if data.len() < offset + user_len {
            return Err(crate::ProtocolError::InvalidPayloadSize(data.len()));
        }
        let user_id =
            UserId::new(String::from_utf8_lossy(&data[offset..offset + user_len]).to_string());
        offset += user_len;

        let data_len = u16::from_le_bytes(data[offset..offset + 2].try_into().unwrap()) as usize;
        offset += 2;

        if data.len() < offset + data_len {
            return Err(crate::ProtocolError::InvalidPayloadSize(data.len()));
        }
        let audio_data = data[offset..offset + data_len].to_vec();

        Ok(Self {
            sequence,
            timestamp,
            channel_id,
            user_id,
            data: audio_data,
            contains_voice,
        })
    }
}

/// Helper module for serializing byte vectors.
mod serde_bytes {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(bytes: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            super::base64::encode(bytes).serialize(serializer)
        } else {
            bytes.serialize(serializer)
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let s = String::deserialize(deserializer)?;
            super::base64::decode(&s).map_err(serde::de::Error::custom)
        } else {
            Vec::<u8>::deserialize(deserializer)
        }
    }
}

/// Placeholder base64 module (will use proper base64 crate).
mod base64 {
    use std::fmt;

    pub fn encode(data: &[u8]) -> String {
        // Simple base64 encoding
        const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

        let mut result = String::new();
        for chunk in data.chunks(3) {
            let n = match chunk.len() {
                3 => ((chunk[0] as u32) << 16) | ((chunk[1] as u32) << 8) | (chunk[2] as u32),
                2 => ((chunk[0] as u32) << 16) | ((chunk[1] as u32) << 8),
                1 => (chunk[0] as u32) << 16,
                _ => unreachable!(),
            };

            result.push(ALPHABET[(n >> 18 & 0x3F) as usize] as char);
            result.push(ALPHABET[(n >> 12 & 0x3F) as usize] as char);

            if chunk.len() > 1 {
                result.push(ALPHABET[(n >> 6 & 0x3F) as usize] as char);
            } else {
                result.push('=');
            }

            if chunk.len() > 2 {
                result.push(ALPHABET[(n & 0x3F) as usize] as char);
            } else {
                result.push('=');
            }
        }
        result
    }

    pub fn decode(s: &str) -> Result<Vec<u8>, DecodeError> {
        const DECODE_TABLE: [i8; 256] = {
            let mut table = [-1i8; 256];
            let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let mut i = 0;
            while i < 64 {
                table[alphabet[i] as usize] = i as i8;
                i += 1;
            }
            table
        };

        let s = s.trim_end_matches('=');
        let mut result = Vec::with_capacity(s.len() * 3 / 4);

        let bytes: Vec<u8> = s
            .bytes()
            .filter_map(|b| {
                let v = DECODE_TABLE[b as usize];
                if v >= 0 {
                    Some(v as u8)
                } else {
                    None
                }
            })
            .collect();

        for chunk in bytes.chunks(4) {
            if chunk.len() >= 2 {
                result.push((chunk[0] << 2) | (chunk[1] >> 4));
            }
            if chunk.len() >= 3 {
                result.push((chunk[1] << 4) | (chunk[2] >> 2));
            }
            if chunk.len() >= 4 {
                result.push((chunk[2] << 6) | chunk[3]);
            }
        }

        Ok(result)
    }

    #[derive(Debug)]
    pub struct DecodeError;

    impl fmt::Display for DecodeError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "base64 decode error")
        }
    }

    impl std::error::Error for DecodeError {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_packet_roundtrip() {
        let packet = AudioPacket::new(
            42,
            12345,
            ChannelId::new("channel1"),
            UserId::new("user1"),
            vec![1, 2, 3, 4, 5],
        );

        let bytes = packet.to_bytes();
        let decoded = AudioPacket::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.sequence, packet.sequence);
        assert_eq!(decoded.timestamp, packet.timestamp);
        assert_eq!(decoded.channel_id, packet.channel_id);
        assert_eq!(decoded.user_id, packet.user_id);
        assert_eq!(decoded.data, packet.data);
        assert_eq!(decoded.contains_voice, packet.contains_voice);
    }

    #[test]
    fn test_client_message_serialization() {
        let msg = ClientMessage::JoinRoom {
            room_id: RoomId::new("room1"),
            user_info: UserInfo::new(UserId::new("user1"), "Test User"),
            password: None,
        };

        let json = serde_json::to_string(&msg).unwrap();
        let decoded: ClientMessage = serde_json::from_str(&json).unwrap();

        match decoded {
            ClientMessage::JoinRoom { room_id, .. } => {
                assert_eq!(room_id.as_str(), "room1");
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_server_message_serialization() {
        let msg = ServerMessage::Welcome {
            protocol_version: PROTOCOL_VERSION,
            user_id: UserId::new("user1"),
            room_info: RoomInfo::new(RoomId::new("room1"), "Test Room"),
        };

        let json = serde_json::to_string(&msg).unwrap();
        let decoded: ServerMessage = serde_json::from_str(&json).unwrap();

        match decoded {
            ServerMessage::Welcome {
                protocol_version, ..
            } => {
                assert_eq!(protocol_version, PROTOCOL_VERSION);
            }
            _ => panic!("Wrong message type"),
        }
    }
}
