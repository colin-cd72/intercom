//! Firebase Realtime Database signaling implementation.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::time::sleep;
use tracing::{debug, error, info, trace, warn};

use crate::{SignalMessage, SignalingError};
use intercom_protocol::{RoomId, UserId};

/// Firebase Realtime Database configuration.
#[derive(Debug, Clone)]
pub struct FirebaseConfig {
    /// Firebase project URL (e.g., "https://your-project.firebaseio.com")
    pub database_url: String,
    /// Optional authentication token.
    pub auth_token: Option<String>,
    /// Polling interval for receiving messages.
    pub poll_interval: Duration,
}

impl Default for FirebaseConfig {
    fn default() -> Self {
        Self {
            database_url: String::new(),
            auth_token: None,
            poll_interval: Duration::from_millis(500),
        }
    }
}

/// Signaling message stored in Firebase.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredMessage {
    #[serde(rename = "type")]
    msg_type: String,
    from: String,
    to: String,
    sdp: Option<String>,
    candidate: Option<String>,
    sdp_mid: Option<String>,
    sdp_mline_index: Option<u16>,
    timestamp: i64,
}

/// Firebase signaling client for WebRTC.
pub struct FirebaseSignaling {
    config: FirebaseConfig,
    client: Client,
    room_id: RoomId,
    user_id: UserId,
    message_tx: Option<mpsc::Sender<(UserId, SignalMessage)>>,
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl FirebaseSignaling {
    /// Create a new Firebase signaling client.
    pub fn new(config: FirebaseConfig, room_id: RoomId, user_id: UserId) -> Self {
        Self {
            config,
            client: Client::new(),
            room_id,
            user_id,
            message_tx: None,
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Get the base URL for the room.
    fn room_url(&self) -> String {
        let auth = self
            .config
            .auth_token
            .as_ref()
            .map(|t| format!("?auth={}", t))
            .unwrap_or_default();
        format!(
            "{}/rooms/{}.json{}",
            self.config.database_url,
            self.room_id.as_str(),
            auth
        )
    }

    /// Get the URL for a user's inbox.
    fn inbox_url(&self, user_id: &UserId) -> String {
        let auth = self
            .config
            .auth_token
            .as_ref()
            .map(|t| format!("?auth={}", t))
            .unwrap_or_default();
        format!(
            "{}/rooms/{}/inbox/{}.json{}",
            self.config.database_url,
            self.room_id.as_str(),
            user_id.as_str(),
            auth
        )
    }

    /// Join the signaling room.
    pub async fn join(&self) -> Result<(), SignalingError> {
        let presence_url = format!(
            "{}/rooms/{}/presence/{}.json{}",
            self.config.database_url,
            self.room_id.as_str(),
            self.user_id.as_str(),
            self.config
                .auth_token
                .as_ref()
                .map(|t| format!("?auth={}", t))
                .unwrap_or_default()
        );

        let presence = serde_json::json!({
            "joined_at": chrono_timestamp(),
            "user_id": self.user_id.as_str(),
        });

        self.client
            .put(&presence_url)
            .json(&presence)
            .send()
            .await?
            .error_for_status()
            .map_err(|e| SignalingError::Firebase(e.to_string()))?;

        info!("Joined signaling room: {}", self.room_id);
        Ok(())
    }

    /// Leave the signaling room.
    pub async fn leave(&self) -> Result<(), SignalingError> {
        let presence_url = format!(
            "{}/rooms/{}/presence/{}.json{}",
            self.config.database_url,
            self.room_id.as_str(),
            self.user_id.as_str(),
            self.config
                .auth_token
                .as_ref()
                .map(|t| format!("?auth={}", t))
                .unwrap_or_default()
        );

        self.client
            .delete(&presence_url)
            .send()
            .await?
            .error_for_status()
            .map_err(|e| SignalingError::Firebase(e.to_string()))?;

        // Clear inbox
        let inbox_url = self.inbox_url(&self.user_id);
        self.client.delete(&inbox_url).send().await?;

        info!("Left signaling room: {}", self.room_id);
        Ok(())
    }

    /// Get list of users in the room.
    pub async fn get_users(&self) -> Result<Vec<UserId>, SignalingError> {
        let presence_url = format!(
            "{}/rooms/{}/presence.json{}",
            self.config.database_url,
            self.room_id.as_str(),
            self.config
                .auth_token
                .as_ref()
                .map(|t| format!("?auth={}", t))
                .unwrap_or_default()
        );

        let response = self.client.get(&presence_url).send().await?;

        if !response.status().is_success() {
            return Ok(Vec::new());
        }

        let users: Option<HashMap<String, serde_json::Value>> = response.json().await?;

        Ok(users
            .unwrap_or_default()
            .keys()
            .map(|k| UserId::new(k.clone()))
            .collect())
    }

    /// Send an SDP offer to a peer.
    pub async fn send_offer(&self, to: &UserId, sdp: &str) -> Result<(), SignalingError> {
        self.send_message(to, "offer", Some(sdp), None).await
    }

    /// Send an SDP answer to a peer.
    pub async fn send_answer(&self, to: &UserId, sdp: &str) -> Result<(), SignalingError> {
        self.send_message(to, "answer", Some(sdp), None).await
    }

    /// Send an ICE candidate to a peer.
    pub async fn send_ice_candidate(
        &self,
        to: &UserId,
        candidate: &str,
        sdp_mid: Option<&str>,
        sdp_mline_index: Option<u16>,
    ) -> Result<(), SignalingError> {
        let msg = StoredMessage {
            msg_type: "ice".to_string(),
            from: self.user_id.to_string(),
            to: to.to_string(),
            sdp: None,
            candidate: Some(candidate.to_string()),
            sdp_mid: sdp_mid.map(|s| s.to_string()),
            sdp_mline_index,
            timestamp: chrono_timestamp(),
        };

        let inbox_url = format!(
            "{}/rooms/{}/inbox/{}.json{}",
            self.config.database_url,
            self.room_id.as_str(),
            to.as_str(),
            self.config
                .auth_token
                .as_ref()
                .map(|t| format!("?auth={}", t))
                .unwrap_or_default()
        );

        self.client
            .post(&inbox_url)
            .json(&msg)
            .send()
            .await?
            .error_for_status()
            .map_err(|e| SignalingError::Firebase(e.to_string()))?;

        trace!("Sent ICE candidate to {}", to);
        Ok(())
    }

    async fn send_message(
        &self,
        to: &UserId,
        msg_type: &str,
        sdp: Option<&str>,
        candidate: Option<&str>,
    ) -> Result<(), SignalingError> {
        let msg = StoredMessage {
            msg_type: msg_type.to_string(),
            from: self.user_id.to_string(),
            to: to.to_string(),
            sdp: sdp.map(|s| s.to_string()),
            candidate: candidate.map(|s| s.to_string()),
            sdp_mid: None,
            sdp_mline_index: None,
            timestamp: chrono_timestamp(),
        };

        let inbox_url = format!(
            "{}/rooms/{}/inbox/{}.json{}",
            self.config.database_url,
            self.room_id.as_str(),
            to.as_str(),
            self.config
                .auth_token
                .as_ref()
                .map(|t| format!("?auth={}", t))
                .unwrap_or_default()
        );

        self.client
            .post(&inbox_url)
            .json(&msg)
            .send()
            .await?
            .error_for_status()
            .map_err(|e| SignalingError::Firebase(e.to_string()))?;

        debug!("Sent {} to {}", msg_type, to);
        Ok(())
    }

    /// Poll for incoming messages.
    pub async fn poll_messages(&self) -> Result<Vec<(UserId, SignalMessage)>, SignalingError> {
        let inbox_url = self.inbox_url(&self.user_id);
        let response = self.client.get(&inbox_url).send().await?;

        if !response.status().is_success() {
            return Ok(Vec::new());
        }

        let messages: Option<HashMap<String, StoredMessage>> = response.json().await?;

        let Some(messages) = messages else {
            return Ok(Vec::new());
        };

        if messages.is_empty() {
            return Ok(Vec::new());
        }

        // Delete processed messages
        self.client.delete(&inbox_url).send().await?;

        let result: Vec<(UserId, SignalMessage)> = messages
            .into_values()
            .filter_map(|msg| {
                let from = UserId::new(&msg.from);
                let signal = match msg.msg_type.as_str() {
                    "offer" => msg.sdp.map(|sdp| SignalMessage::Offer { sdp }),
                    "answer" => msg.sdp.map(|sdp| SignalMessage::Answer { sdp }),
                    "ice" => msg.candidate.map(|candidate| SignalMessage::IceCandidate {
                        candidate,
                        sdp_mid: msg.sdp_mid,
                        sdp_mline_index: msg.sdp_mline_index,
                    }),
                    _ => None,
                };
                signal.map(|s| (from, s))
            })
            .collect();

        if !result.is_empty() {
            debug!("Received {} signaling messages", result.len());
        }

        Ok(result)
    }

    /// Start the message polling loop.
    pub fn start_polling(&mut self) -> mpsc::Receiver<(UserId, SignalMessage)> {
        let (tx, rx) = mpsc::channel(100);
        self.message_tx = Some(tx.clone());
        self.running
            .store(true, std::sync::atomic::Ordering::Relaxed);

        let config = self.config.clone();
        let room_id = self.room_id.clone();
        let user_id = self.user_id.clone();
        let running = self.running.clone();
        let client = self.client.clone();

        tokio::spawn(async move {
            let signaling = FirebaseSignaling {
                config: config.clone(),
                client,
                room_id,
                user_id,
                message_tx: None,
                running: running.clone(),
            };

            while running.load(std::sync::atomic::Ordering::Relaxed) {
                match signaling.poll_messages().await {
                    Ok(messages) => {
                        for msg in messages {
                            if tx.send(msg).await.is_err() {
                                warn!("Message receiver dropped");
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error polling messages: {}", e);
                    }
                }
                sleep(config.poll_interval).await;
            }
        });

        rx
    }

    /// Stop the message polling loop.
    pub fn stop_polling(&self) {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }
}

impl Drop for FirebaseSignaling {
    fn drop(&mut self) {
        self.stop_polling();
    }
}

/// Get current timestamp in milliseconds.
fn chrono_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = FirebaseConfig::default();
        assert!(config.database_url.is_empty());
        assert!(config.auth_token.is_none());
    }

    #[test]
    fn test_signaling_creation() {
        let config = FirebaseConfig {
            database_url: "https://test.firebaseio.com".to_string(),
            ..Default::default()
        };

        let signaling = FirebaseSignaling::new(
            config,
            RoomId::new("test-room"),
            UserId::new("test-user"),
        );

        assert_eq!(signaling.room_id.as_str(), "test-room");
        assert_eq!(signaling.user_id.as_str(), "test-user");
    }
}
