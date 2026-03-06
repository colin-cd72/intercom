//! Opus codec wrapper for the intercom system.
//!
//! This crate provides encoding and decoding of audio using the Opus codec.

pub mod opus;

pub use opus::{OpusDecoder, OpusEncoder};

use thiserror::Error;

/// Codec errors.
#[derive(Debug, Error)]
pub enum CodecError {
    #[error("Failed to create encoder: {0}")]
    EncoderCreation(String),

    #[error("Failed to create decoder: {0}")]
    DecoderCreation(String),

    #[error("Encoding error: {0}")]
    Encoding(String),

    #[error("Decoding error: {0}")]
    Decoding(String),

    #[error("Invalid frame size: {0}")]
    InvalidFrameSize(usize),

    #[error("Buffer too small: need {needed}, have {available}")]
    BufferTooSmall { needed: usize, available: usize },
}

/// Encoded audio frame.
#[derive(Debug, Clone)]
pub struct EncodedFrame {
    /// The encoded data.
    pub data: Vec<u8>,
    /// Number of samples encoded.
    pub samples: usize,
    /// Whether this frame contains voice (if VAD is enabled).
    pub has_voice: bool,
}

impl EncodedFrame {
    pub fn new(data: Vec<u8>, samples: usize) -> Self {
        Self {
            data,
            samples,
            has_voice: true,
        }
    }

    pub fn with_voice(mut self, has_voice: bool) -> Self {
        self.has_voice = has_voice;
        self
    }
}

/// Decoded audio frame.
#[derive(Debug, Clone)]
pub struct DecodedFrame {
    /// The decoded samples (f32, mono).
    pub samples: Vec<f32>,
    /// Whether this was a FEC (forward error correction) frame.
    pub is_fec: bool,
}

impl DecodedFrame {
    pub fn new(samples: Vec<f32>) -> Self {
        Self {
            samples,
            is_fec: false,
        }
    }

    pub fn fec(samples: Vec<f32>) -> Self {
        Self {
            samples,
            is_fec: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoded_frame() {
        let frame = EncodedFrame::new(vec![1, 2, 3], 480).with_voice(true);
        assert_eq!(frame.data, vec![1, 2, 3]);
        assert_eq!(frame.samples, 480);
        assert!(frame.has_voice);
    }

    #[test]
    fn test_decoded_frame() {
        let frame = DecodedFrame::new(vec![0.1, 0.2, 0.3]);
        assert_eq!(frame.samples.len(), 3);
        assert!(!frame.is_fec);

        let fec_frame = DecodedFrame::fec(vec![0.1, 0.2]);
        assert!(fec_frame.is_fec);
    }
}
