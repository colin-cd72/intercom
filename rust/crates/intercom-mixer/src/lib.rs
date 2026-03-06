//! Audio mixing and jitter buffer for the intercom system.

pub mod channel_mixer;
pub mod jitter_buffer;

pub use channel_mixer::ChannelMixer;
pub use jitter_buffer::JitterBuffer;

use thiserror::Error;

/// Mixer errors.
#[derive(Debug, Error)]
pub enum MixerError {
    #[error("Buffer overflow")]
    BufferOverflow,

    #[error("Buffer underflow")]
    BufferUnderflow,

    #[error("Invalid sequence number")]
    InvalidSequence,

    #[error("Channel not found: {0}")]
    ChannelNotFound(String),

    #[error("User not found: {0}")]
    UserNotFound(String),
}

/// Mixed audio output with metadata.
#[derive(Debug, Clone)]
pub struct MixedAudio {
    /// The mixed audio samples.
    pub samples: Vec<f32>,
    /// List of users who contributed to this mix.
    pub speakers: Vec<String>,
    /// Timestamp of the oldest sample in the mix.
    pub timestamp: u64,
}

impl MixedAudio {
    pub fn new(samples: Vec<f32>) -> Self {
        Self {
            samples,
            speakers: Vec::new(),
            timestamp: 0,
        }
    }

    pub fn silence(num_samples: usize) -> Self {
        Self {
            samples: vec![0.0; num_samples],
            speakers: Vec::new(),
            timestamp: 0,
        }
    }
}
