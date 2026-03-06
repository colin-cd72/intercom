//! Intercom server implementation.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::config::{DeviceSelector, ServerConfig};
use crate::events::{EventHandler, IntercomEvent};
use crate::{ChannelId, ChannelInfo, CoreError, RoomId, UserId, UserInfo};
use intercom_audio::AudioHandle;
use intercom_codec::OpusEncoder;
use intercom_crypto::Cipher;
use intercom_mixer::ChannelMixer;
use intercom_session::{RoomSession, SessionManager};
use intercom_signaling::FirebaseSignaling;
use intercom_transport::PeerConnection;

/// Intercom server for hosting rooms.
pub struct IntercomServer {
    config: ServerConfig,
    room_id: RoomId,
    session_manager: SessionManager,
    event_handler: Arc<RwLock<Option<Arc<dyn EventHandler>>>>,

    // Audio components (using thread-safe handle)
    audio: Option<AudioHandle>,
    encoder: Option<OpusEncoder>,
    mixer: ChannelMixer,

    // Networking
    signaling: Option<FirebaseSignaling>,
    peer_connections: Arc<RwLock<HashMap<UserId, Arc<PeerConnection>>>>,
    cipher: Cipher,

    // State
    running: Arc<AtomicBool>,
}

impl IntercomServer {
    /// Create a new intercom server.
    pub fn new(config: ServerConfig) -> Result<Self, CoreError> {
        let room_id = RoomId::new(uuid_v4());

        // Generate or use provided encryption key
        let cipher = if let Some(key) = &config.encryption_key {
            Cipher::from_key(key)?
        } else {
            Cipher::new()
        };

        Ok(Self {
            config,
            room_id,
            session_manager: SessionManager::new(),
            event_handler: Arc::new(RwLock::new(None)),
            audio: None,
            encoder: None,
            mixer: ChannelMixer::new(),
            signaling: None,
            peer_connections: Arc::new(RwLock::new(HashMap::new())),
            cipher,
            running: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Set the event handler.
    pub fn set_event_handler<H: EventHandler + 'static>(&self, handler: H) {
        *self.event_handler.write() = Some(Arc::new(handler));
    }

    /// Get the room ID.
    pub fn room_id(&self) -> &RoomId {
        &self.room_id
    }

    /// Get the encryption key for sharing with clients.
    pub fn encryption_key(&self) -> &[u8] {
        self.cipher.key_bytes()
    }

    /// Initialize audio devices for server monitoring.
    pub fn init_audio(&mut self) -> Result<(), CoreError> {
        // Select devices
        let input_device = DeviceSelector::select_input(
            self.config.input_device.as_deref(),
            self.config.prefer_dante,
        )?;
        let output_device = DeviceSelector::select_output(
            self.config.output_device.as_deref(),
            self.config.prefer_dante,
        )?;

        info!("Server using input device: {}", input_device);
        info!("Server using output device: {}", output_device);

        // Create thread-safe audio handle
        self.audio = Some(AudioHandle::new(
            Some(&input_device),
            Some(&output_device),
        )?);
        self.encoder = Some(OpusEncoder::new()?);

        Ok(())
    }

    /// Start the server.
    pub async fn start(&mut self) -> Result<(), CoreError> {
        info!("Starting intercom server: {}", self.config.name);

        // Create room session
        let mut room = RoomSession::new(self.room_id.clone(), &self.config.name);
        room.max_users = self.config.max_users;

        if let Some(password) = &self.config.password {
            // In production, use proper password hashing
            room = room.with_password(password.clone());
        }

        self.session_manager.create_room(room);

        // Create default channels
        if self.config.auto_create_channels {
            for (i, name) in self.config.default_channels.iter().enumerate() {
                let channel_id = ChannelId::new(format!("channel-{}", i));
                let channel = ChannelInfo::new(channel_id.clone(), name);
                self.session_manager.create_channel(&self.room_id, channel)?;
                self.mixer.add_channel(&channel_id);
                info!("Created channel: {}", name);
            }
        }

        // Initialize signaling
        let signaling = FirebaseSignaling::new(
            self.config.firebase.clone(),
            self.room_id.clone(),
            UserId::new("server"),
        );

        signaling.join().await?;
        self.signaling = Some(signaling);

        self.running.store(true, Ordering::Relaxed);

        // Start audio if configured
        if let Some(ref audio) = self.audio {
            audio.start_playback()?;
        }

        info!("Server started. Room ID: {}", self.room_id);
        Ok(())
    }

    /// Stop the server.
    pub async fn stop(&mut self) -> Result<(), CoreError> {
        info!("Stopping intercom server");
        self.running.store(false, Ordering::Relaxed);

        // Close all peer connections
        let connections: Vec<_> = self.peer_connections.write().drain().collect();
        for (user_id, pc) in connections {
            info!("Disconnecting user: {}", user_id);
            let _ = pc.close().await;
        }

        // Leave signaling
        if let Some(signaling) = &self.signaling {
            let _ = signaling.leave().await;
        }

        // Stop audio
        if let Some(ref audio) = self.audio {
            let _ = audio.stop_capture();
            let _ = audio.stop_playback();
        }

        // Delete room
        let _ = self.session_manager.delete_room(&self.room_id);

        info!("Server stopped");
        Ok(())
    }

    /// Check if server is running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Get list of connected users.
    pub fn get_users(&self) -> Result<Vec<UserInfo>, CoreError> {
        self.session_manager.get_room_users(&self.room_id).map_err(Into::into)
    }

    /// Get list of channels.
    pub fn get_channels(&self) -> Result<Vec<ChannelInfo>, CoreError> {
        self.session_manager.get_room_channels(&self.room_id).map_err(Into::into)
    }

    /// Create a new channel.
    pub fn create_channel(&self, name: impl Into<String>) -> Result<ChannelId, CoreError> {
        let channel_id = ChannelId::new(uuid_v4());
        let channel = ChannelInfo::new(channel_id.clone(), name);
        self.session_manager.create_channel(&self.room_id, channel.clone())?;
        self.mixer.add_channel(&channel_id);

        self.emit_event(IntercomEvent::ChannelCreated(channel));
        Ok(channel_id)
    }

    /// Delete a channel.
    pub fn delete_channel(&self, channel_id: &ChannelId) -> Result<(), CoreError> {
        self.session_manager.delete_channel(&self.room_id, channel_id)?;
        self.mixer.remove_channel(channel_id);

        self.emit_event(IntercomEvent::ChannelDeleted(channel_id.clone()));
        Ok(())
    }

    /// Kick a user from the server.
    pub async fn kick_user(&self, user_id: &UserId) -> Result<(), CoreError> {
        // Close peer connection
        if let Some(pc) = self.peer_connections.write().remove(user_id) {
            let _ = pc.close().await;
        }

        // Remove from session
        let _ = self.session_manager.leave_room(user_id);

        self.emit_event(IntercomEvent::UserLeft(user_id.clone()));
        info!("Kicked user: {}", user_id);
        Ok(())
    }

    /// Mute/unmute a user.
    pub fn set_user_muted(
        &self,
        channel_id: &ChannelId,
        user_id: &UserId,
        muted: bool,
    ) -> Result<(), CoreError> {
        self.mixer.set_user_muted(channel_id, user_id, muted)
            .map_err(|e| CoreError::InvalidConfig(e.to_string()))
    }

    /// Set user volume.
    pub fn set_user_volume(
        &self,
        channel_id: &ChannelId,
        user_id: &UserId,
        volume: f32,
    ) -> Result<(), CoreError> {
        self.mixer.set_user_gain(channel_id, user_id, volume)
            .map_err(|e| CoreError::InvalidConfig(e.to_string()))
    }

    /// Get channel statistics.
    pub fn get_channel_stats(&self, channel_id: &ChannelId) -> ChannelStats {
        let subscribers = self.session_manager
            .get_channel_subscribers(&self.room_id, channel_id)
            .unwrap_or_default();
        let talkers = self.session_manager
            .get_channel_talkers(&self.room_id, channel_id)
            .unwrap_or_default();

        ChannelStats {
            channel_id: channel_id.clone(),
            subscriber_count: subscribers.len(),
            talker_count: talkers.len(),
            active_talkers: talkers,
        }
    }

    /// Run cleanup for idle users.
    pub fn cleanup_idle(&self, max_idle_secs: u64) {
        let removed = self.session_manager.cleanup_idle_users(Duration::from_secs(max_idle_secs));
        for (room_id, user_id) in removed {
            self.emit_event(IntercomEvent::UserLeft(user_id));
        }
    }

    fn emit_event(&self, event: IntercomEvent) {
        if let Some(handler) = self.event_handler.read().as_ref() {
            handler.on_event(event);
        }
    }
}

/// Channel statistics.
#[derive(Debug, Clone)]
pub struct ChannelStats {
    pub channel_id: ChannelId,
    pub subscriber_count: usize,
    pub talker_count: usize,
    pub active_talkers: Vec<UserId>,
}

/// Generate a simple UUID v4.
fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    // Use timestamp and xor for pseudo-random parts
    let high: u64 = (timestamp >> 64) as u64 ^ 0xfedcba9876543210;
    let low: u64 = timestamp as u64 ^ 0x123456789abcdef0;

    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        (high >> 32) as u32,
        (high >> 16) as u16,
        high as u16 & 0x0fff,
        ((low >> 48) as u16 & 0x3fff) | 0x8000,
        low & 0xffffffffffff
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_creation() {
        let config = ServerConfig::new("Test Server");
        let server = IntercomServer::new(config);
        assert!(server.is_ok());
    }

    #[test]
    fn test_encryption_key() {
        let config = ServerConfig::new("Test Server");
        let server = IntercomServer::new(config).unwrap();
        assert_eq!(server.encryption_key().len(), 32);
    }
}
