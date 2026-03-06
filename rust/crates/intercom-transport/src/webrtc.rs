//! WebRTC peer connection implementation.

use std::sync::Arc;

use crossbeam_channel::{bounded, Receiver, Sender};
use parking_lot::RwLock;
use tokio::sync::mpsc;
use tracing::{debug, error, info, trace, warn};
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::RTCDataChannel;
use webrtc::ice_transport::ice_candidate::RTCIceCandidate;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;

use crate::{ConnectionState, TransportError, STUN_SERVERS};
use intercom_protocol::UserId;

/// Transport configuration.
#[derive(Debug, Clone)]
pub struct TransportConfig {
    /// STUN server URLs.
    pub stun_servers: Vec<String>,
    /// Optional TURN server URLs with credentials.
    pub turn_servers: Vec<TurnServer>,
    /// Ordered/reliable data channel.
    pub ordered: bool,
    /// Max retransmits (for unreliable mode).
    pub max_retransmits: Option<u16>,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            stun_servers: STUN_SERVERS.iter().map(|s| s.to_string()).collect(),
            turn_servers: Vec::new(),
            ordered: false,         // Unordered for lower latency
            max_retransmits: Some(0), // No retransmits for real-time audio
        }
    }
}

/// TURN server configuration.
#[derive(Debug, Clone)]
pub struct TurnServer {
    pub urls: Vec<String>,
    pub username: String,
    pub credential: String,
}

/// ICE candidate for signaling.
#[derive(Debug, Clone)]
pub struct IceCandidate {
    pub candidate: String,
    pub sdp_mid: Option<String>,
    pub sdp_mline_index: Option<u16>,
}

/// WebRTC peer connection wrapper.
pub struct PeerConnection {
    peer_id: UserId,
    connection: Arc<RTCPeerConnection>,
    data_channel: Arc<RwLock<Option<Arc<RTCDataChannel>>>>,
    state: Arc<RwLock<ConnectionState>>,
    ice_candidates: Arc<RwLock<Vec<IceCandidate>>>,
    incoming_tx: Sender<Vec<u8>>,
    incoming_rx: Receiver<Vec<u8>>,
    ice_tx: mpsc::Sender<IceCandidate>,
    config: TransportConfig,
}

impl PeerConnection {
    /// Create a new peer connection.
    pub async fn new(
        peer_id: UserId,
        config: TransportConfig,
    ) -> Result<(Self, mpsc::Receiver<IceCandidate>), TransportError> {
        // Build ICE servers configuration
        let mut ice_servers = vec![RTCIceServer {
            urls: config.stun_servers.clone(),
            ..Default::default()
        }];

        for turn in &config.turn_servers {
            ice_servers.push(RTCIceServer {
                urls: turn.urls.clone(),
                username: turn.username.clone(),
                credential: turn.credential.clone(),
                ..Default::default()
            });
        }

        let rtc_config = RTCConfiguration {
            ice_servers,
            ..Default::default()
        };

        // Create media engine and interceptor registry
        let mut media_engine = MediaEngine::default();
        media_engine
            .register_default_codecs()
            .map_err(|e| TransportError::WebRTC(e.to_string()))?;

        let mut registry = Registry::new();
        registry = register_default_interceptors(registry, &mut media_engine)
            .map_err(|e| TransportError::WebRTC(e.to_string()))?;

        // Build API
        let api = APIBuilder::new()
            .with_media_engine(media_engine)
            .with_interceptor_registry(registry)
            .build();

        // Create peer connection
        let connection = api
            .new_peer_connection(rtc_config)
            .await
            .map_err(|e| TransportError::WebRTC(e.to_string()))?;

        let connection = Arc::new(connection);
        let state = Arc::new(RwLock::new(ConnectionState::New));
        let ice_candidates = Arc::new(RwLock::new(Vec::new()));
        let data_channel = Arc::new(RwLock::new(None));
        let (incoming_tx, incoming_rx) = bounded(256);
        let (ice_tx, ice_rx) = mpsc::channel(64);

        // Set up connection state handler
        let state_clone = state.clone();
        connection.on_peer_connection_state_change(Box::new(move |s| {
            let state = state_clone.clone();
            Box::pin(async move {
                let new_state = match s {
                    RTCPeerConnectionState::New => ConnectionState::New,
                    RTCPeerConnectionState::Connecting => ConnectionState::Connecting,
                    RTCPeerConnectionState::Connected => ConnectionState::Connected,
                    RTCPeerConnectionState::Disconnected => ConnectionState::Disconnected,
                    RTCPeerConnectionState::Failed => ConnectionState::Failed,
                    RTCPeerConnectionState::Closed => ConnectionState::Closed,
                    _ => ConnectionState::New,
                };
                info!("Peer connection state changed: {}", new_state);
                *state.write() = new_state;
            })
        }));

        // Set up ICE candidate handler
        let ice_candidates_clone = ice_candidates.clone();
        let ice_tx_clone = ice_tx.clone();
        connection.on_ice_candidate(Box::new(move |candidate| {
            let ice_candidates = ice_candidates_clone.clone();
            let ice_tx = ice_tx_clone.clone();
            Box::pin(async move {
                if let Some(c) = candidate {
                    let candidate_json = match c.to_json() {
                        Ok(j) => j,
                        Err(e) => {
                            error!("Failed to serialize ICE candidate: {}", e);
                            return;
                        }
                    };

                    let ice = IceCandidate {
                        candidate: candidate_json.candidate,
                        sdp_mid: candidate_json.sdp_mid,
                        sdp_mline_index: candidate_json.sdp_mline_index,
                    };

                    debug!("New ICE candidate: {}", ice.candidate);
                    ice_candidates.write().push(ice.clone());
                    let _ = ice_tx.send(ice).await;
                }
            })
        }));

        // Set up data channel handler for incoming channels
        let data_channel_clone = data_channel.clone();
        let incoming_tx_clone = incoming_tx.clone();
        connection.on_data_channel(Box::new(move |dc| {
            let data_channel = data_channel_clone.clone();
            let incoming_tx = incoming_tx_clone.clone();

            Box::pin(async move {
                info!("Data channel opened: {}", dc.label());
                *data_channel.write() = Some(dc.clone());

                dc.on_message(Box::new(move |msg| {
                    let tx = incoming_tx.clone();
                    Box::pin(async move {
                        trace!("Received {} bytes", msg.data.len());
                        if tx.try_send(msg.data.to_vec()).is_err() {
                            warn!("Incoming data buffer full");
                        }
                    })
                }));
            })
        }));

        Ok((
            Self {
                peer_id,
                connection,
                data_channel,
                state,
                ice_candidates,
                incoming_tx,
                incoming_rx,
                ice_tx,
                config,
            },
            ice_rx,
        ))
    }

    /// Create an offer (for initiator).
    pub async fn create_offer(&self) -> Result<String, TransportError> {
        // Create data channel
        let dc = self
            .connection
            .create_data_channel("audio", None)
            .await
            .map_err(|e| TransportError::WebRTC(e.to_string()))?;

        // Set up data channel handlers
        let incoming_tx = self.incoming_tx.clone();
        dc.on_message(Box::new(move |msg| {
            let tx = incoming_tx.clone();
            Box::pin(async move {
                trace!("Received {} bytes", msg.data.len());
                if tx.try_send(msg.data.to_vec()).is_err() {
                    warn!("Incoming data buffer full");
                }
            })
        }));

        let data_channel = self.data_channel.clone();
        let dc_clone = dc.clone();
        dc.on_open(Box::new(move || {
            let data_channel = data_channel.clone();
            let dc = dc_clone.clone();
            Box::pin(async move {
                info!("Data channel opened");
                *data_channel.write() = Some(dc);
            })
        }));

        // Create offer
        let offer = self
            .connection
            .create_offer(None)
            .await
            .map_err(|e| TransportError::WebRTC(e.to_string()))?;

        // Set local description
        self.connection
            .set_local_description(offer.clone())
            .await
            .map_err(|e| TransportError::WebRTC(e.to_string()))?;

        info!("Created offer for peer: {}", self.peer_id);
        Ok(offer.sdp)
    }

    /// Handle an incoming offer and create an answer (for responder).
    pub async fn handle_offer(&self, sdp: &str) -> Result<String, TransportError> {
        let offer = RTCSessionDescription::offer(sdp.to_string())
            .map_err(|e| TransportError::InvalidSdp(e.to_string()))?;

        self.connection
            .set_remote_description(offer)
            .await
            .map_err(|e| TransportError::WebRTC(e.to_string()))?;

        let answer = self
            .connection
            .create_answer(None)
            .await
            .map_err(|e| TransportError::WebRTC(e.to_string()))?;

        self.connection
            .set_local_description(answer.clone())
            .await
            .map_err(|e| TransportError::WebRTC(e.to_string()))?;

        info!("Created answer for peer: {}", self.peer_id);
        Ok(answer.sdp)
    }

    /// Handle an incoming answer.
    pub async fn handle_answer(&self, sdp: &str) -> Result<(), TransportError> {
        let answer = RTCSessionDescription::answer(sdp.to_string())
            .map_err(|e| TransportError::InvalidSdp(e.to_string()))?;

        self.connection
            .set_remote_description(answer)
            .await
            .map_err(|e| TransportError::WebRTC(e.to_string()))?;

        info!("Accepted answer from peer: {}", self.peer_id);
        Ok(())
    }

    /// Add a remote ICE candidate.
    pub async fn add_ice_candidate(&self, candidate: &IceCandidate) -> Result<(), TransportError> {
        use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;

        let init = RTCIceCandidateInit {
            candidate: candidate.candidate.clone(),
            sdp_mid: candidate.sdp_mid.clone(),
            sdp_mline_index: candidate.sdp_mline_index,
            ..Default::default()
        };

        self.connection
            .add_ice_candidate(init)
            .await
            .map_err(|e| TransportError::WebRTC(e.to_string()))?;

        trace!("Added ICE candidate");
        Ok(())
    }

    /// Send data to the peer.
    pub async fn send(&self, data: &[u8]) -> Result<(), TransportError> {
        let dc = self
            .data_channel
            .read()
            .clone()
            .ok_or(TransportError::NotConnected)?;

        dc.send(&bytes::Bytes::copy_from_slice(data))
            .await
            .map_err(|e| TransportError::WebRTC(e.to_string()))?;

        trace!("Sent {} bytes", data.len());
        Ok(())
    }

    /// Receive data from the peer (non-blocking).
    pub fn try_recv(&self) -> Option<Vec<u8>> {
        self.incoming_rx.try_recv().ok()
    }

    /// Receive data from the peer (blocking).
    pub fn recv(&self) -> Result<Vec<u8>, TransportError> {
        self.incoming_rx
            .recv()
            .map_err(|_| TransportError::ChannelClosed)
    }

    /// Get the receiver for incoming data.
    pub fn receiver(&self) -> &Receiver<Vec<u8>> {
        &self.incoming_rx
    }

    /// Get the current connection state.
    pub fn state(&self) -> ConnectionState {
        *self.state.read()
    }

    /// Check if the connection is established.
    pub fn is_connected(&self) -> bool {
        *self.state.read() == ConnectionState::Connected
    }

    /// Get the peer ID.
    pub fn peer_id(&self) -> &UserId {
        &self.peer_id
    }

    /// Get gathered ICE candidates.
    pub fn ice_candidates(&self) -> Vec<IceCandidate> {
        self.ice_candidates.read().clone()
    }

    /// Close the connection.
    pub async fn close(&self) -> Result<(), TransportError> {
        self.connection
            .close()
            .await
            .map_err(|e| TransportError::WebRTC(e.to_string()))?;
        info!("Closed connection to peer: {}", self.peer_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_peer_connection_creation() {
        let config = TransportConfig::default();
        let result = PeerConnection::new(UserId::new("test-peer"), config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_create_offer() {
        let config = TransportConfig::default();
        let (pc, _ice_rx) = PeerConnection::new(UserId::new("test-peer"), config)
            .await
            .unwrap();

        let offer = pc.create_offer().await;
        assert!(offer.is_ok());
        assert!(offer.unwrap().contains("v=0"));
    }
}
