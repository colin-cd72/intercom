//! Configuration types for the intercom system.

use std::time::Duration;

use intercom_signaling::FirebaseConfig;
use intercom_transport::TransportConfig;

/// Client configuration.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// User display name.
    pub display_name: String,
    /// Input device name (None for default, use "Dante" prefix for DANTE devices).
    pub input_device: Option<String>,
    /// Output device name (None for default, use "Dante" prefix for DANTE devices).
    pub output_device: Option<String>,
    /// Firebase configuration for signaling.
    pub firebase: FirebaseConfig,
    /// Transport configuration.
    pub transport: TransportConfig,
    /// Enable echo cancellation.
    pub echo_cancellation: bool,
    /// Enable noise suppression.
    pub noise_suppression: bool,
    /// Enable automatic gain control.
    pub auto_gain_control: bool,
    /// Push-to-talk mode (vs voice activity detection).
    pub push_to_talk: bool,
    /// Voice activity detection threshold (0.0 - 1.0).
    pub vad_threshold: f32,
    /// Connection timeout.
    pub connect_timeout: Duration,
    /// Prefer DANTE devices when available.
    pub prefer_dante: bool,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            display_name: "User".to_string(),
            input_device: None,
            output_device: None,
            firebase: FirebaseConfig::default(),
            transport: TransportConfig::default(),
            echo_cancellation: true,
            noise_suppression: true,
            auto_gain_control: true,
            push_to_talk: true,
            vad_threshold: 0.01,
            connect_timeout: Duration::from_secs(30),
            prefer_dante: true,
        }
    }
}

impl ClientConfig {
    /// Create a new client configuration with display name.
    pub fn new(display_name: impl Into<String>) -> Self {
        Self {
            display_name: display_name.into(),
            ..Default::default()
        }
    }

    /// Set Firebase database URL.
    pub fn with_firebase_url(mut self, url: impl Into<String>) -> Self {
        self.firebase.database_url = url.into();
        self
    }

    /// Set input device.
    pub fn with_input_device(mut self, device: impl Into<String>) -> Self {
        self.input_device = Some(device.into());
        self
    }

    /// Set output device.
    pub fn with_output_device(mut self, device: impl Into<String>) -> Self {
        self.output_device = Some(device.into());
        self
    }

    /// Enable/disable push-to-talk mode.
    pub fn with_push_to_talk(mut self, enabled: bool) -> Self {
        self.push_to_talk = enabled;
        self
    }

    /// Set whether to prefer DANTE devices.
    pub fn with_prefer_dante(mut self, prefer: bool) -> Self {
        self.prefer_dante = prefer;
        self
    }
}

/// Server configuration.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Server name/room name.
    pub name: String,
    /// Room password (optional).
    pub password: Option<String>,
    /// Firebase configuration for signaling.
    pub firebase: FirebaseConfig,
    /// Maximum number of users.
    pub max_users: usize,
    /// Input device name (for server audio monitoring).
    pub input_device: Option<String>,
    /// Output device name (for server audio monitoring).
    pub output_device: Option<String>,
    /// Auto-create default channels.
    pub auto_create_channels: bool,
    /// Default channel names.
    pub default_channels: Vec<String>,
    /// Encryption key (auto-generated if None).
    pub encryption_key: Option<[u8; 32]>,
    /// Prefer DANTE devices when available.
    pub prefer_dante: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            name: "Intercom Server".to_string(),
            password: None,
            firebase: FirebaseConfig::default(),
            max_users: 50,
            input_device: None,
            output_device: None,
            auto_create_channels: true,
            default_channels: vec![
                "Main".to_string(),
                "Channel 1".to_string(),
                "Channel 2".to_string(),
            ],
            encryption_key: None,
            prefer_dante: true,
        }
    }
}

impl ServerConfig {
    /// Create a new server configuration with name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Set Firebase database URL.
    pub fn with_firebase_url(mut self, url: impl Into<String>) -> Self {
        self.firebase.database_url = url.into();
        self
    }

    /// Set room password.
    pub fn with_password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(password.into());
        self
    }

    /// Set maximum users.
    pub fn with_max_users(mut self, max: usize) -> Self {
        self.max_users = max;
        self
    }

    /// Set default channels.
    pub fn with_channels(mut self, channels: Vec<String>) -> Self {
        self.default_channels = channels;
        self
    }

    /// Set whether to prefer DANTE devices.
    pub fn with_prefer_dante(mut self, prefer: bool) -> Self {
        self.prefer_dante = prefer;
        self
    }
}

/// Audio device selection helper.
pub struct DeviceSelector;

impl DeviceSelector {
    /// Find the best input device, preferring DANTE if configured.
    pub fn select_input(
        preferred: Option<&str>,
        prefer_dante: bool,
    ) -> Result<String, crate::CoreError> {
        use intercom_audio::{AudioDevice, DeviceType};

        let devices = AudioDevice::list_inputs()
            .map_err(|e| crate::CoreError::Audio(e))?;

        // If specific device requested, try to find it
        if let Some(name) = preferred {
            if devices.iter().any(|d| d.name == name) {
                return Ok(name.to_string());
            }
        }

        // If prefer DANTE, look for DANTE devices
        if prefer_dante {
            if let Some(dante) = devices.iter().find(|d| {
                d.name.to_lowercase().contains("dante") ||
                d.name.to_lowercase().contains("dvs")
            }) {
                return Ok(dante.name.clone());
            }
        }

        // Fall back to default
        devices
            .iter()
            .find(|d| d.is_default)
            .or(devices.first())
            .map(|d| d.name.clone())
            .ok_or_else(|| crate::CoreError::Audio(intercom_audio::AudioError::NoDevice))
    }

    /// Find the best output device, preferring DANTE if configured.
    pub fn select_output(
        preferred: Option<&str>,
        prefer_dante: bool,
    ) -> Result<String, crate::CoreError> {
        use intercom_audio::{AudioDevice, DeviceType};

        let devices = AudioDevice::list_outputs()
            .map_err(|e| crate::CoreError::Audio(e))?;

        // If specific device requested, try to find it
        if let Some(name) = preferred {
            if devices.iter().any(|d| d.name == name) {
                return Ok(name.to_string());
            }
        }

        // If prefer DANTE, look for DANTE devices
        if prefer_dante {
            if let Some(dante) = devices.iter().find(|d| {
                d.name.to_lowercase().contains("dante") ||
                d.name.to_lowercase().contains("dvs")
            }) {
                return Ok(dante.name.clone());
            }
        }

        // Fall back to default
        devices
            .iter()
            .find(|d| d.is_default)
            .or(devices.first())
            .map(|d| d.name.clone())
            .ok_or_else(|| crate::CoreError::Audio(intercom_audio::AudioError::NoDevice))
    }

    /// List all available input devices with DANTE priority indication.
    pub fn list_inputs_with_priority() -> Result<Vec<(String, bool, bool)>, crate::CoreError> {
        use intercom_audio::AudioDevice;

        let devices = AudioDevice::list_inputs()
            .map_err(|e| crate::CoreError::Audio(e))?;

        Ok(devices
            .iter()
            .map(|d| {
                let is_dante = d.name.to_lowercase().contains("dante") ||
                               d.name.to_lowercase().contains("dvs");
                (d.name.clone(), is_dante, d.is_default)
            })
            .collect())
    }

    /// List all available output devices with DANTE priority indication.
    pub fn list_outputs_with_priority() -> Result<Vec<(String, bool, bool)>, crate::CoreError> {
        use intercom_audio::AudioDevice;

        let devices = AudioDevice::list_outputs()
            .map_err(|e| crate::CoreError::Audio(e))?;

        Ok(devices
            .iter()
            .map(|d| {
                let is_dante = d.name.to_lowercase().contains("dante") ||
                               d.name.to_lowercase().contains("dvs");
                (d.name.clone(), is_dante, d.is_default)
            })
            .collect())
    }
}
