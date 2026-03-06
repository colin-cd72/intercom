//! FFI bindings for the intercom system.
//!
//! This crate provides UniFFI bindings for Swift and Kotlin,
//! and C bindings via cbindgen for C++.
//!
//! The FFI layer uses message passing to avoid Send/Sync issues with
//! audio streams and codec raw pointers.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use crossbeam_channel::{bounded, Receiver, Sender};
use parking_lot::RwLock;

// Re-exports for UniFFI
pub use intercom_core::{init_logging, version};

uniffi::setup_scaffolding!();

// ============================================================================
// Types for FFI
// ============================================================================

/// Audio device information for FFI.
#[derive(Debug, Clone, uniffi::Record)]
pub struct AudioDeviceInfo {
    pub name: String,
    pub is_dante: bool,
    pub is_default: bool,
}

/// Client configuration for FFI.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ClientConfiguration {
    pub display_name: String,
    pub input_device: Option<String>,
    pub output_device: Option<String>,
    pub firebase_url: String,
    pub push_to_talk: bool,
    pub prefer_dante: bool,
}

/// Server configuration for FFI.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ServerConfiguration {
    pub name: String,
    pub password: Option<String>,
    pub firebase_url: String,
    pub max_users: u32,
    pub default_channels: Vec<String>,
    pub prefer_dante: bool,
}

/// Channel information for FFI.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ChannelInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

/// User information for FFI.
#[derive(Debug, Clone, uniffi::Record)]
pub struct UserInfo {
    pub id: String,
    pub display_name: String,
    pub is_talking: bool,
}

/// Connection state for FFI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ConnectionState {
    New,
    Connecting,
    Connected,
    Disconnected,
    Failed,
    Closed,
}

/// Intercom events for FFI.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum IntercomEvent {
    ConnectionStateChanged { state: ConnectionState },
    Connected { room_id: String, user_id: String },
    Disconnected { reason: String },
    UserJoined { user: UserInfo },
    UserLeft { user_id: String },
    UserTalkStart { user_id: String, channel_id: String },
    UserTalkStop { user_id: String },
    ChannelCreated { channel: ChannelInfo },
    ChannelDeleted { channel_id: String },
    TalkStarted { channel_id: String },
    TalkStopped,
    AudioLevel { input_level: f32, output_level: f32 },
    Error { message: String },
}

/// Error type for FFI.
#[derive(Debug, Clone, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum IntercomError {
    #[error("Audio error: {message}")]
    AudioError { message: String },
    #[error("Connection error: {message}")]
    ConnectionError { message: String },
    #[error("Signaling error: {message}")]
    SignalingError { message: String },
    #[error("Not connected: {message}")]
    NotConnected { message: String },
    #[error("Already connected: {message}")]
    AlreadyConnected { message: String },
    #[error("Invalid config: {message}")]
    InvalidConfig { message: String },
    #[error("Timeout: {message}")]
    Timeout { message: String },
    #[error("Internal error: {message}")]
    Internal { message: String },
}


impl From<intercom_core::CoreError> for IntercomError {
    fn from(e: intercom_core::CoreError) -> Self {
        match e {
            intercom_core::CoreError::Audio(e) => IntercomError::AudioError { message: e.to_string() },
            intercom_core::CoreError::Transport(e) => IntercomError::ConnectionError { message: e.to_string() },
            intercom_core::CoreError::Signaling(e) => IntercomError::SignalingError { message: e.to_string() },
            intercom_core::CoreError::NotConnected => IntercomError::NotConnected { message: "Not connected".to_string() },
            intercom_core::CoreError::AlreadyConnected => IntercomError::AlreadyConnected { message: "Already connected".to_string() },
            intercom_core::CoreError::InvalidConfig(s) => IntercomError::InvalidConfig { message: s },
            intercom_core::CoreError::Timeout => IntercomError::Timeout { message: "Operation timed out".to_string() },
            _ => IntercomError::ConnectionError { message: e.to_string() },
        }
    }
}

// Note: Events are polled using poll_events() or get_next_event() methods.
// Callback interfaces are not used to avoid FFI complexity.

// ============================================================================
// Namespace functions
// ============================================================================

/// Get the intercom version.
#[uniffi::export]
pub fn get_version() -> String {
    version().to_string()
}

/// List input devices.
#[uniffi::export]
pub fn list_input_devices() -> Vec<AudioDeviceInfo> {
    use intercom_audio::AudioDevice;

    AudioDevice::list_inputs()
        .unwrap_or_default()
        .into_iter()
        .map(|d| AudioDeviceInfo {
            name: d.name.clone(),
            is_dante: d.name.to_lowercase().contains("dante"),
            is_default: d.is_default,
        })
        .collect()
}

/// List output devices.
#[uniffi::export]
pub fn list_output_devices() -> Vec<AudioDeviceInfo> {
    use intercom_audio::AudioDevice;

    AudioDevice::list_outputs()
        .unwrap_or_default()
        .into_iter()
        .map(|d| AudioDeviceInfo {
            name: d.name.clone(),
            is_dante: d.name.to_lowercase().contains("dante"),
            is_default: d.is_default,
        })
        .collect()
}

// ============================================================================
// Client commands and responses
// ============================================================================

enum ClientCommand {
    InitAudio,
    SetInputDevice(String),
    SetOutputDevice(String),
    Connect(String),
    Disconnect,
    SubscribeChannel(String),
    UnsubscribeChannel(String),
    StartTalk(String),
    StopTalk,
    StartAudio,
    StopAudio,
    GetUserId,
    GetRoomId,
    IsConnected,
    IsTalking,
    GetTalkChannel,
    Shutdown,
}

enum ClientResponse {
    Ok,
    Error(String),
    UserId(String),
    RoomId(Option<String>),
    Bool(bool),
    TalkChannel(Option<String>),
}

// ============================================================================
// Client wrapper (Send + Sync safe)
// ============================================================================

/// FFI wrapper for the intercom client.
///
/// This wrapper runs the actual client on a dedicated thread and communicates
/// via channels, ensuring all operations are thread-safe.
#[derive(uniffi::Object)]
pub struct IntercomClient {
    command_tx: Sender<ClientCommand>,
    response_rx: Receiver<ClientResponse>,
    event_rx: Receiver<IntercomEvent>,
    running: Arc<AtomicBool>,
    thread_handle: RwLock<Option<JoinHandle<()>>>,
}

#[uniffi::export]
impl IntercomClient {
    /// Create a new intercom client.
    #[uniffi::constructor]
    pub fn new(config: ClientConfiguration) -> Result<Arc<Self>, IntercomError> {
        let (command_tx, command_rx) = bounded::<ClientCommand>(64);
        let (response_tx, response_rx) = bounded::<ClientResponse>(64);
        let (event_tx, event_rx) = bounded::<IntercomEvent>(256);
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        // Spawn the client thread
        let thread_handle = thread::Builder::new()
            .name("intercom-client".to_string())
            .spawn(move || {
                client_thread(config, command_rx, response_tx, event_tx, running_clone);
            })
            .map_err(|e| IntercomError::Internal { message: format!("Failed to spawn client thread: {}", e) })?;

        Ok(Arc::new(Self {
            command_tx,
            response_rx,
            event_rx,
            running,
            thread_handle: RwLock::new(Some(thread_handle)),
        }))
    }

    /// Get the next event (non-blocking).
    /// Returns None if no events are available.
    pub fn get_next_event(&self) -> Option<IntercomEvent> {
        self.event_rx.try_recv().ok()
    }

    /// Get all pending events.
    pub fn get_pending_events(&self) -> Vec<IntercomEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.event_rx.try_recv() {
            events.push(event);
        }
        events
    }

    /// Get the user ID.
    pub fn get_user_id(&self) -> String {
        self.send_command(ClientCommand::GetUserId);
        match self.receive_response() {
            ClientResponse::UserId(id) => id,
            _ => String::new(),
        }
    }

    /// Get the current room ID.
    pub fn get_room_id(&self) -> Option<String> {
        self.send_command(ClientCommand::GetRoomId);
        match self.receive_response() {
            ClientResponse::RoomId(id) => id,
            _ => None,
        }
    }

    /// Check if connected.
    pub fn is_connected(&self) -> bool {
        self.send_command(ClientCommand::IsConnected);
        match self.receive_response() {
            ClientResponse::Bool(b) => b,
            _ => false,
        }
    }

    /// Initialize audio devices.
    pub fn init_audio(&self) -> Result<(), IntercomError> {
        self.send_command(ClientCommand::InitAudio);
        self.check_response()
    }

    /// Set input device.
    pub fn set_input_device(&self, device_name: String) -> Result<(), IntercomError> {
        self.send_command(ClientCommand::SetInputDevice(device_name));
        self.check_response()
    }

    /// Set output device.
    pub fn set_output_device(&self, device_name: String) -> Result<(), IntercomError> {
        self.send_command(ClientCommand::SetOutputDevice(device_name));
        self.check_response()
    }

    /// Connect to a room.
    pub fn connect(&self, room_id: String) -> Result<(), IntercomError> {
        self.send_command(ClientCommand::Connect(room_id));
        self.check_response()
    }

    /// Disconnect from the room.
    pub fn disconnect(&self) -> Result<(), IntercomError> {
        self.send_command(ClientCommand::Disconnect);
        self.check_response()
    }

    /// Subscribe to a channel.
    pub fn subscribe_channel(&self, channel_id: String) {
        self.send_command(ClientCommand::SubscribeChannel(channel_id));
        let _ = self.receive_response();
    }

    /// Unsubscribe from a channel.
    pub fn unsubscribe_channel(&self, channel_id: String) {
        self.send_command(ClientCommand::UnsubscribeChannel(channel_id));
        let _ = self.receive_response();
    }

    /// Start talking on a channel.
    pub fn start_talk(&self, channel_id: String) -> Result<(), IntercomError> {
        self.send_command(ClientCommand::StartTalk(channel_id));
        self.check_response()
    }

    /// Stop talking.
    pub fn stop_talk(&self) -> Result<(), IntercomError> {
        self.send_command(ClientCommand::StopTalk);
        self.check_response()
    }

    /// Check if currently talking.
    pub fn is_talking(&self) -> bool {
        self.send_command(ClientCommand::IsTalking);
        match self.receive_response() {
            ClientResponse::Bool(b) => b,
            _ => false,
        }
    }

    /// Get the current talk channel.
    pub fn get_talk_channel(&self) -> Option<String> {
        self.send_command(ClientCommand::GetTalkChannel);
        match self.receive_response() {
            ClientResponse::TalkChannel(c) => c,
            _ => None,
        }
    }

    /// Start audio processing.
    pub fn start_audio(&self) -> Result<(), IntercomError> {
        self.send_command(ClientCommand::StartAudio);
        self.check_response()
    }

    /// Stop audio processing.
    pub fn stop_audio(&self) -> Result<(), IntercomError> {
        self.send_command(ClientCommand::StopAudio);
        self.check_response()
    }
}

impl IntercomClient {
    fn send_command(&self, cmd: ClientCommand) {
        let _ = self.command_tx.send(cmd);
    }

    fn receive_response(&self) -> ClientResponse {
        self.response_rx
            .recv_timeout(std::time::Duration::from_secs(30))
            .unwrap_or(ClientResponse::Error("Timeout".to_string()))
    }

    fn check_response(&self) -> Result<(), IntercomError> {
        match self.receive_response() {
            ClientResponse::Ok => Ok(()),
            ClientResponse::Error(e) => Err(IntercomError::Internal { message: e }),
            _ => Ok(()),
        }
    }
}

impl Drop for IntercomClient {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        let _ = self.command_tx.send(ClientCommand::Shutdown);

        if let Some(handle) = self.thread_handle.write().take() {
            let _ = handle.join();
        }
    }
}

/// Client thread function.
fn client_thread(
    config: ClientConfiguration,
    command_rx: Receiver<ClientCommand>,
    response_tx: Sender<ClientResponse>,
    event_tx: Sender<IntercomEvent>,
    running: Arc<AtomicBool>,
) {
    // Create tokio runtime for async operations
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            let _ = response_tx.send(ClientResponse::Error(e.to_string()));
            return;
        }
    };

    // Create core config
    let core_config = intercom_core::ClientConfig {
        display_name: config.display_name,
        input_device: config.input_device,
        output_device: config.output_device,
        firebase: intercom_signaling::FirebaseConfig {
            database_url: config.firebase_url,
            ..Default::default()
        },
        push_to_talk: config.push_to_talk,
        prefer_dante: config.prefer_dante,
        ..Default::default()
    };

    // Create client
    let mut client = match intercom_core::IntercomClient::new(core_config) {
        Ok(c) => c,
        Err(e) => {
            let _ = response_tx.send(ClientResponse::Error(e.to_string()));
            return;
        }
    };

    while running.load(Ordering::Relaxed) {
        match command_rx.recv_timeout(std::time::Duration::from_millis(10)) {
            Ok(cmd) => {
                let response = handle_client_command(&mut client, cmd, &runtime, &event_tx);
                if matches!(&response, ClientResponse::Error(e) if e == "shutdown") {
                    break;
                }
                let _ = response_tx.send(response);
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                // No command, continue
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }
}

fn handle_client_command(
    client: &mut intercom_core::IntercomClient,
    cmd: ClientCommand,
    runtime: &tokio::runtime::Runtime,
    _event_tx: &Sender<IntercomEvent>,
) -> ClientResponse {
    match cmd {
        ClientCommand::InitAudio => {
            match client.init_audio() {
                Ok(()) => ClientResponse::Ok,
                Err(e) => ClientResponse::Error(e.to_string()),
            }
        }
        ClientCommand::SetInputDevice(name) => {
            match client.set_input_device(&name) {
                Ok(()) => ClientResponse::Ok,
                Err(e) => ClientResponse::Error(e.to_string()),
            }
        }
        ClientCommand::SetOutputDevice(name) => {
            match client.set_output_device(&name) {
                Ok(()) => ClientResponse::Ok,
                Err(e) => ClientResponse::Error(e.to_string()),
            }
        }
        ClientCommand::Connect(room_id) => {
            let room = intercom_core::RoomId::new(room_id);
            match runtime.block_on(client.connect(room)) {
                Ok(()) => ClientResponse::Ok,
                Err(e) => ClientResponse::Error(e.to_string()),
            }
        }
        ClientCommand::Disconnect => {
            match runtime.block_on(client.disconnect()) {
                Ok(()) => ClientResponse::Ok,
                Err(e) => ClientResponse::Error(e.to_string()),
            }
        }
        ClientCommand::SubscribeChannel(id) => {
            client.subscribe_channel(intercom_core::ChannelId::new(id));
            ClientResponse::Ok
        }
        ClientCommand::UnsubscribeChannel(id) => {
            client.unsubscribe_channel(&intercom_core::ChannelId::new(id));
            ClientResponse::Ok
        }
        ClientCommand::StartTalk(id) => {
            match client.start_talk(intercom_core::ChannelId::new(id)) {
                Ok(()) => ClientResponse::Ok,
                Err(e) => ClientResponse::Error(e.to_string()),
            }
        }
        ClientCommand::StopTalk => {
            match client.stop_talk() {
                Ok(()) => ClientResponse::Ok,
                Err(e) => ClientResponse::Error(e.to_string()),
            }
        }
        ClientCommand::StartAudio => {
            match client.start_audio() {
                Ok(()) => ClientResponse::Ok,
                Err(e) => ClientResponse::Error(e.to_string()),
            }
        }
        ClientCommand::StopAudio => {
            match client.stop_audio() {
                Ok(()) => ClientResponse::Ok,
                Err(e) => ClientResponse::Error(e.to_string()),
            }
        }
        ClientCommand::GetUserId => {
            ClientResponse::UserId(client.user_id().to_string())
        }
        ClientCommand::GetRoomId => {
            ClientResponse::RoomId(client.room_id().map(|r| r.to_string()))
        }
        ClientCommand::IsConnected => {
            ClientResponse::Bool(client.is_connected())
        }
        ClientCommand::IsTalking => {
            ClientResponse::Bool(client.is_talking())
        }
        ClientCommand::GetTalkChannel => {
            ClientResponse::TalkChannel(client.talk_channel().map(|c| c.to_string()))
        }
        ClientCommand::Shutdown => {
            ClientResponse::Error("shutdown".to_string())
        }
    }
}

// ============================================================================
// Server commands and responses
// ============================================================================

enum ServerCommand {
    InitAudio,
    Start,
    Stop,
    GetRoomId,
    GetEncryptionKey,
    IsRunning,
    GetUsers,
    GetChannels,
    CreateChannel(String),
    DeleteChannel(String),
    KickUser(String),
    Shutdown,
}

enum ServerResponse {
    Ok,
    Error(String),
    RoomId(String),
    EncryptionKey(Vec<u8>),
    Bool(bool),
    Users(Vec<UserInfo>),
    Channels(Vec<ChannelInfo>),
    ChannelId(String),
}

// ============================================================================
// Server wrapper (Send + Sync safe)
// ============================================================================

/// FFI wrapper for the intercom server.
#[derive(uniffi::Object)]
pub struct IntercomServer {
    command_tx: Sender<ServerCommand>,
    response_rx: Receiver<ServerResponse>,
    event_rx: Receiver<IntercomEvent>,
    running: Arc<AtomicBool>,
    thread_handle: RwLock<Option<JoinHandle<()>>>,
}

#[uniffi::export]
impl IntercomServer {
    /// Create a new intercom server.
    #[uniffi::constructor]
    pub fn new(config: ServerConfiguration) -> Result<Arc<Self>, IntercomError> {
        let (command_tx, command_rx) = bounded::<ServerCommand>(64);
        let (response_tx, response_rx) = bounded::<ServerResponse>(64);
        let (event_tx, event_rx) = bounded::<IntercomEvent>(256);
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        // Spawn the server thread
        let thread_handle = thread::Builder::new()
            .name("intercom-server".to_string())
            .spawn(move || {
                server_thread(config, command_rx, response_tx, event_tx, running_clone);
            })
            .map_err(|e| IntercomError::Internal { message: format!("Failed to spawn server thread: {}", e) })?;

        Ok(Arc::new(Self {
            command_tx,
            response_rx,
            event_rx,
            running,
            thread_handle: RwLock::new(Some(thread_handle)),
        }))
    }

    /// Get the next event (non-blocking).
    /// Returns None if no events are available.
    pub fn get_next_event(&self) -> Option<IntercomEvent> {
        self.event_rx.try_recv().ok()
    }

    /// Get all pending events.
    pub fn get_pending_events(&self) -> Vec<IntercomEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.event_rx.try_recv() {
            events.push(event);
        }
        events
    }

    /// Get the room ID.
    pub fn get_room_id(&self) -> String {
        self.send_command(ServerCommand::GetRoomId);
        match self.receive_response() {
            ServerResponse::RoomId(id) => id,
            _ => String::new(),
        }
    }

    /// Get the encryption key.
    pub fn get_encryption_key(&self) -> Vec<u8> {
        self.send_command(ServerCommand::GetEncryptionKey);
        match self.receive_response() {
            ServerResponse::EncryptionKey(key) => key,
            _ => Vec::new(),
        }
    }

    /// Check if server is running.
    pub fn is_running(&self) -> bool {
        self.send_command(ServerCommand::IsRunning);
        match self.receive_response() {
            ServerResponse::Bool(b) => b,
            _ => false,
        }
    }

    /// Initialize audio devices.
    pub fn init_audio(&self) -> Result<(), IntercomError> {
        self.send_command(ServerCommand::InitAudio);
        self.check_response()
    }

    /// Start the server.
    pub fn start(&self) -> Result<(), IntercomError> {
        self.send_command(ServerCommand::Start);
        self.check_response()
    }

    /// Stop the server.
    pub fn stop(&self) -> Result<(), IntercomError> {
        self.send_command(ServerCommand::Stop);
        self.check_response()
    }

    /// Get list of users.
    pub fn get_users(&self) -> Vec<UserInfo> {
        self.send_command(ServerCommand::GetUsers);
        match self.receive_response() {
            ServerResponse::Users(users) => users,
            _ => Vec::new(),
        }
    }

    /// Get list of channels.
    pub fn get_channels(&self) -> Vec<ChannelInfo> {
        self.send_command(ServerCommand::GetChannels);
        match self.receive_response() {
            ServerResponse::Channels(channels) => channels,
            _ => Vec::new(),
        }
    }

    /// Create a new channel.
    pub fn create_channel(&self, name: String) -> Result<String, IntercomError> {
        self.send_command(ServerCommand::CreateChannel(name));
        match self.receive_response() {
            ServerResponse::ChannelId(id) => Ok(id),
            ServerResponse::Error(e) => Err(IntercomError::Internal { message: e }),
            _ => Err(IntercomError::Internal { message: "Unexpected response".to_string() }),
        }
    }

    /// Delete a channel.
    pub fn delete_channel(&self, channel_id: String) -> Result<(), IntercomError> {
        self.send_command(ServerCommand::DeleteChannel(channel_id));
        self.check_response()
    }

    /// Kick a user.
    pub fn kick_user(&self, user_id: String) -> Result<(), IntercomError> {
        self.send_command(ServerCommand::KickUser(user_id));
        self.check_response()
    }
}

impl IntercomServer {
    fn send_command(&self, cmd: ServerCommand) {
        let _ = self.command_tx.send(cmd);
    }

    fn receive_response(&self) -> ServerResponse {
        self.response_rx
            .recv_timeout(std::time::Duration::from_secs(30))
            .unwrap_or(ServerResponse::Error("Timeout".to_string()))
    }

    fn check_response(&self) -> Result<(), IntercomError> {
        match self.receive_response() {
            ServerResponse::Ok => Ok(()),
            ServerResponse::Error(e) => Err(IntercomError::Internal { message: e }),
            _ => Ok(()),
        }
    }
}

impl Drop for IntercomServer {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        let _ = self.command_tx.send(ServerCommand::Shutdown);

        if let Some(handle) = self.thread_handle.write().take() {
            let _ = handle.join();
        }
    }
}

/// Server thread function.
fn server_thread(
    config: ServerConfiguration,
    command_rx: Receiver<ServerCommand>,
    response_tx: Sender<ServerResponse>,
    _event_tx: Sender<IntercomEvent>,
    running: Arc<AtomicBool>,
) {
    // Create tokio runtime for async operations
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            let _ = response_tx.send(ServerResponse::Error(e.to_string()));
            return;
        }
    };

    // Create core config
    let core_config = intercom_core::ServerConfig {
        name: config.name,
        password: config.password,
        firebase: intercom_signaling::FirebaseConfig {
            database_url: config.firebase_url,
            ..Default::default()
        },
        max_users: config.max_users as usize,
        default_channels: config.default_channels,
        prefer_dante: config.prefer_dante,
        ..Default::default()
    };

    // Create server
    let mut server = match intercom_core::IntercomServer::new(core_config) {
        Ok(s) => s,
        Err(e) => {
            let _ = response_tx.send(ServerResponse::Error(e.to_string()));
            return;
        }
    };

    while running.load(Ordering::Relaxed) {
        match command_rx.recv_timeout(std::time::Duration::from_millis(10)) {
            Ok(cmd) => {
                let response = handle_server_command(&mut server, cmd, &runtime);
                if matches!(&response, ServerResponse::Error(e) if e == "shutdown") {
                    break;
                }
                let _ = response_tx.send(response);
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                // No command, continue
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }
}

fn handle_server_command(
    server: &mut intercom_core::IntercomServer,
    cmd: ServerCommand,
    runtime: &tokio::runtime::Runtime,
) -> ServerResponse {
    match cmd {
        ServerCommand::InitAudio => {
            match server.init_audio() {
                Ok(()) => ServerResponse::Ok,
                Err(e) => ServerResponse::Error(e.to_string()),
            }
        }
        ServerCommand::Start => {
            match runtime.block_on(server.start()) {
                Ok(()) => ServerResponse::Ok,
                Err(e) => ServerResponse::Error(e.to_string()),
            }
        }
        ServerCommand::Stop => {
            match runtime.block_on(server.stop()) {
                Ok(()) => ServerResponse::Ok,
                Err(e) => ServerResponse::Error(e.to_string()),
            }
        }
        ServerCommand::GetRoomId => {
            ServerResponse::RoomId(server.room_id().to_string())
        }
        ServerCommand::GetEncryptionKey => {
            ServerResponse::EncryptionKey(server.encryption_key().to_vec())
        }
        ServerCommand::IsRunning => {
            ServerResponse::Bool(server.is_running())
        }
        ServerCommand::GetUsers => {
            let users = server
                .get_users()
                .unwrap_or_default()
                .into_iter()
                .map(|u| UserInfo {
                    id: u.id.to_string(),
                    display_name: u.display_name,
                    is_talking: u.is_talking,
                })
                .collect();
            ServerResponse::Users(users)
        }
        ServerCommand::GetChannels => {
            let channels = server
                .get_channels()
                .unwrap_or_default()
                .into_iter()
                .map(|c| ChannelInfo {
                    id: c.id.to_string(),
                    name: c.name,
                    description: c.description,
                })
                .collect();
            ServerResponse::Channels(channels)
        }
        ServerCommand::CreateChannel(name) => {
            match server.create_channel(name) {
                Ok(id) => ServerResponse::ChannelId(id.to_string()),
                Err(e) => ServerResponse::Error(e.to_string()),
            }
        }
        ServerCommand::DeleteChannel(id) => {
            match server.delete_channel(&intercom_core::ChannelId::new(id)) {
                Ok(()) => ServerResponse::Ok,
                Err(e) => ServerResponse::Error(e.to_string()),
            }
        }
        ServerCommand::KickUser(id) => {
            match runtime.block_on(server.kick_user(&intercom_core::UserId::new(id))) {
                Ok(()) => ServerResponse::Ok,
                Err(e) => ServerResponse::Error(e.to_string()),
            }
        }
        ServerCommand::Shutdown => {
            ServerResponse::Error("shutdown".to_string())
        }
    }
}
