//! Intercom client implementation.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use crossbeam_channel::{Receiver, Sender};
use parking_lot::RwLock;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::config::{ClientConfig, DeviceSelector};
use crate::events::{EventHandler, IntercomEvent};
use crate::{ChannelId, CoreError, RoomId, UserId};
use intercom_audio::AudioHandle;
use intercom_audio::vad::Vad;
use intercom_codec::{OpusDecoder, OpusEncoder};
use intercom_crypto::Cipher;
use intercom_mixer::ChannelMixer;
use intercom_signaling::FirebaseSignaling;
use intercom_transport::{ConnectionState, PeerConnection};

/// Intercom client for connecting to a server.
pub struct IntercomClient {
    config: ClientConfig,
    user_id: UserId,
    state: Arc<RwLock<ClientState>>,
    event_handler: Arc<RwLock<Option<Arc<dyn EventHandler>>>>,

    // Audio components (using thread-safe handle)
    audio: Option<AudioHandle>,
    encoder: Option<OpusEncoder>,
    decoder: Option<OpusDecoder>,
    vad: Vad,
    mixer: ChannelMixer,

    // Networking
    signaling: Option<FirebaseSignaling>,
    peer_connections: Arc<RwLock<HashMap<UserId, Arc<PeerConnection>>>>,
    cipher: Option<Cipher>,

    // State
    room_id: Arc<RwLock<Option<RoomId>>>,
    subscribed_channels: Arc<RwLock<Vec<ChannelId>>>,
    talk_channel: Arc<RwLock<Option<ChannelId>>>,
    is_talking: AtomicBool,
    sequence: AtomicU32,

    // Shutdown
    running: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClientState {
    Disconnected,
    Connecting,
    Connected,
    Error,
}

impl IntercomClient {
    /// Create a new intercom client.
    pub fn new(config: ClientConfig) -> Result<Self, CoreError> {
        let user_id = UserId::new(uuid_v4());

        Ok(Self {
            config,
            user_id,
            state: Arc::new(RwLock::new(ClientState::Disconnected)),
            event_handler: Arc::new(RwLock::new(None)),
            audio: None,
            encoder: None,
            decoder: None,
            vad: Vad::default(),
            mixer: ChannelMixer::new(),
            signaling: None,
            peer_connections: Arc::new(RwLock::new(HashMap::new())),
            cipher: None,
            room_id: Arc::new(RwLock::new(None)),
            subscribed_channels: Arc::new(RwLock::new(Vec::new())),
            talk_channel: Arc::new(RwLock::new(None)),
            is_talking: AtomicBool::new(false),
            sequence: AtomicU32::new(0),
            running: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Set the event handler.
    pub fn set_event_handler<H: EventHandler + 'static>(&self, handler: H) {
        *self.event_handler.write() = Some(Arc::new(handler));
    }

    /// Get the user ID.
    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }

    /// Get the current room ID.
    pub fn room_id(&self) -> Option<RoomId> {
        self.room_id.read().clone()
    }

    /// Check if connected.
    pub fn is_connected(&self) -> bool {
        *self.state.read() == ClientState::Connected
    }

    /// Initialize audio devices.
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

        info!("Using input device: {}", input_device);
        info!("Using output device: {}", output_device);

        // Create thread-safe audio handle
        self.audio = Some(AudioHandle::new(
            Some(&input_device),
            Some(&output_device),
        )?);

        // Create codec
        self.encoder = Some(OpusEncoder::new()?);
        self.decoder = Some(OpusDecoder::new()?);

        Ok(())
    }

    /// List available input devices.
    pub fn list_input_devices() -> Result<Vec<(String, bool, bool)>, CoreError> {
        DeviceSelector::list_inputs_with_priority()
    }

    /// List available output devices.
    pub fn list_output_devices() -> Result<Vec<(String, bool, bool)>, CoreError> {
        DeviceSelector::list_outputs_with_priority()
    }

    /// Set input device.
    pub fn set_input_device(&mut self, device_name: &str) -> Result<(), CoreError> {
        if let Some(ref audio) = self.audio {
            audio.set_input_device(device_name)?;
        }
        Ok(())
    }

    /// Set output device.
    pub fn set_output_device(&mut self, device_name: &str) -> Result<(), CoreError> {
        if let Some(ref audio) = self.audio {
            audio.set_output_device(device_name)?;
        }
        Ok(())
    }

    /// Connect to a room.
    pub async fn connect(&mut self, room_id: RoomId) -> Result<(), CoreError> {
        if *self.state.read() != ClientState::Disconnected {
            return Err(CoreError::AlreadyConnected);
        }

        *self.state.write() = ClientState::Connecting;
        self.emit_event(IntercomEvent::ConnectionStateChanged(ConnectionState::Connecting));

        // Initialize signaling
        let signaling = FirebaseSignaling::new(
            self.config.firebase.clone(),
            room_id.clone(),
            self.user_id.clone(),
        );

        // Join signaling room
        signaling.join().await?;

        // Get existing users
        let users = signaling.get_users().await?;
        info!("Found {} users in room", users.len());

        // Set up peer connections with existing users
        for peer_id in users {
            if peer_id != self.user_id {
                self.create_peer_connection(&peer_id).await?;
            }
        }

        self.signaling = Some(signaling);
        *self.room_id.write() = Some(room_id.clone());
        *self.state.write() = ClientState::Connected;
        self.running.store(true, Ordering::Relaxed);

        self.emit_event(IntercomEvent::Connected {
            room_id: room_id.clone(),
            user_id: self.user_id.clone(),
        });
        self.emit_event(IntercomEvent::RoomJoined {
            room_id,
            room_name: "Room".to_string(),
        });

        Ok(())
    }

    async fn create_peer_connection(&self, peer_id: &UserId) -> Result<(), CoreError> {
        let (pc, mut ice_rx) = PeerConnection::new(
            peer_id.clone(),
            self.config.transport.clone(),
        ).await?;

        let pc = Arc::new(pc);

        // Create offer
        let offer_sdp = pc.create_offer().await?;

        // Send offer via signaling
        if let Some(signaling) = &self.signaling {
            signaling.send_offer(peer_id, &offer_sdp).await?;
        }

        // Store connection
        self.peer_connections.write().insert(peer_id.clone(), pc.clone());

        // Handle ICE candidates in background
        let signaling_clone = self.signaling.as_ref().map(|_| ());
        let peer_id_clone = peer_id.clone();

        tokio::spawn(async move {
            while let Some(candidate) = ice_rx.recv().await {
                // Send ICE candidate via signaling
                debug!("Would send ICE candidate to {}", peer_id_clone);
            }
        });

        Ok(())
    }

    /// Disconnect from the room.
    pub async fn disconnect(&mut self) -> Result<(), CoreError> {
        self.running.store(false, Ordering::Relaxed);

        // Close peer connections
        let connections: Vec<_> = self.peer_connections.write().drain().collect();
        for (_, pc) in connections {
            let _ = pc.close().await;
        }

        // Leave signaling room
        if let Some(signaling) = &self.signaling {
            let _ = signaling.leave().await;
        }

        // Stop audio
        if let Some(ref audio) = self.audio {
            let _ = audio.stop_capture();
            let _ = audio.stop_playback();
        }

        let room_id = self.room_id.write().take();
        *self.state.write() = ClientState::Disconnected;

        if let Some(room_id) = room_id {
            self.emit_event(IntercomEvent::RoomLeft { room_id });
        }
        self.emit_event(IntercomEvent::Disconnected {
            reason: "User disconnected".to_string(),
        });

        Ok(())
    }

    /// Subscribe to a channel.
    pub fn subscribe_channel(&self, channel_id: ChannelId) {
        self.subscribed_channels.write().push(channel_id.clone());
        self.mixer.add_channel(&channel_id);
        self.emit_event(IntercomEvent::ChannelSubscribed(channel_id));
    }

    /// Unsubscribe from a channel.
    pub fn unsubscribe_channel(&self, channel_id: &ChannelId) {
        self.subscribed_channels.write().retain(|c| c != channel_id);
        self.mixer.remove_channel(channel_id);
        self.emit_event(IntercomEvent::ChannelUnsubscribed(channel_id.clone()));
    }

    /// Start talking on a channel.
    pub fn start_talk(&self, channel_id: ChannelId) -> Result<(), CoreError> {
        if !self.is_connected() {
            return Err(CoreError::NotConnected);
        }

        *self.talk_channel.write() = Some(channel_id.clone());
        self.is_talking.store(true, Ordering::Relaxed);

        // Start audio capture
        if let Some(ref audio) = self.audio {
            audio.start_capture()?;
        }

        self.emit_event(IntercomEvent::TalkStarted { channel_id });
        Ok(())
    }

    /// Stop talking.
    pub fn stop_talk(&self) -> Result<(), CoreError> {
        self.is_talking.store(false, Ordering::Relaxed);

        // Stop audio capture
        if let Some(ref audio) = self.audio {
            audio.stop_capture()?;
        }

        *self.talk_channel.write() = None;
        self.emit_event(IntercomEvent::TalkStopped);
        Ok(())
    }

    /// Check if currently talking.
    pub fn is_talking(&self) -> bool {
        self.is_talking.load(Ordering::Relaxed)
    }

    /// Get the current talk channel.
    pub fn talk_channel(&self) -> Option<ChannelId> {
        self.talk_channel.read().clone()
    }

    /// Start audio processing.
    pub fn start_audio(&self) -> Result<(), CoreError> {
        if let Some(ref audio) = self.audio {
            audio.start_playback()?;
        }
        Ok(())
    }

    /// Stop audio processing.
    pub fn stop_audio(&self) -> Result<(), CoreError> {
        if let Some(ref audio) = self.audio {
            audio.stop_capture()?;
            audio.stop_playback()?;
        }
        Ok(())
    }

    fn emit_event(&self, event: IntercomEvent) {
        if let Some(handler) = self.event_handler.read().as_ref() {
            handler.on_event(event);
        }
    }
}

impl Drop for IntercomClient {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

/// Generate a simple UUID v4.
fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    // Use timestamp and xor for pseudo-random parts
    let high: u64 = (timestamp >> 64) as u64 ^ 0x123456789abcdef0;
    let low: u64 = timestamp as u64 ^ 0xfedcba9876543210;

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
    fn test_client_creation() {
        let config = ClientConfig::new("Test User");
        let client = IntercomClient::new(config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_uuid_generation() {
        let id1 = uuid_v4();
        let id2 = uuid_v4();
        assert_ne!(id1, id2);
        assert_eq!(id1.len(), 36);
    }
}
