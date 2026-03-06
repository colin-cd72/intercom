//! Opus codec implementation.

use std::convert::TryFrom;

use audiopus::{
    coder::{Decoder as OpusDecoderInner, Encoder as OpusEncoderInner, GenericCtl},
    packet::Packet,
    Application, Bitrate, Channels, MutSignals, SampleRate, Signal,
};
use tracing::{debug, trace};

use crate::{CodecError, DecodedFrame, EncodedFrame};
use intercom_protocol::audio_params;

/// Opus encoder for voice audio.
pub struct OpusEncoder {
    encoder: OpusEncoderInner,
    frame_size: usize,
    encode_buffer: Vec<u8>,
}

impl OpusEncoder {
    /// Create a new Opus encoder with default settings.
    pub fn new() -> Result<Self, CodecError> {
        Self::with_config(audio_params::SAMPLE_RATE, audio_params::OPUS_BITRATE)
    }

    /// Create a new Opus encoder with custom settings.
    pub fn with_config(sample_rate: u32, bitrate: i32) -> Result<Self, CodecError> {
        let opus_sample_rate = match sample_rate {
            8000 => SampleRate::Hz8000,
            12000 => SampleRate::Hz12000,
            16000 => SampleRate::Hz16000,
            24000 => SampleRate::Hz24000,
            48000 => SampleRate::Hz48000,
            _ => {
                return Err(CodecError::EncoderCreation(format!(
                    "Unsupported sample rate: {}",
                    sample_rate
                )))
            }
        };

        let mut encoder = OpusEncoderInner::new(opus_sample_rate, Channels::Mono, Application::Voip)
            .map_err(|e| CodecError::EncoderCreation(e.to_string()))?;

        // Configure encoder for low-latency voice
        encoder
            .set_bitrate(Bitrate::BitsPerSecond(bitrate))
            .map_err(|e| CodecError::EncoderCreation(e.to_string()))?;

        encoder
            .set_signal(Signal::Voice)
            .map_err(|e| CodecError::EncoderCreation(e.to_string()))?;

        // Enable DTX (discontinuous transmission) for efficiency
        encoder
            .set_dtx(true)
            .map_err(|e| CodecError::EncoderCreation(e.to_string()))?;

        // Enable inband FEC for packet loss resilience
        encoder
            .set_inband_fec(true)
            .map_err(|e| CodecError::EncoderCreation(e.to_string()))?;

        debug!(
            "Created Opus encoder: {}Hz, {}bps, VoIP mode",
            sample_rate, bitrate
        );

        Ok(Self {
            encoder,
            frame_size: audio_params::FRAME_SIZE,
            encode_buffer: vec![0u8; 4000], // Max Opus frame size
        })
    }

    /// Encode a frame of audio samples.
    ///
    /// Input should be f32 samples normalized to -1.0..1.0.
    /// Returns the encoded Opus frame.
    pub fn encode(&mut self, samples: &[f32]) -> Result<EncodedFrame, CodecError> {
        if samples.len() != self.frame_size {
            return Err(CodecError::InvalidFrameSize(samples.len()));
        }

        // Convert f32 to i16 for Opus
        let pcm: Vec<i16> = samples
            .iter()
            .map(|&s| (s * 32767.0).clamp(-32768.0, 32767.0) as i16)
            .collect();

        let encoded_len = self
            .encoder
            .encode(&pcm, &mut self.encode_buffer)
            .map_err(|e| CodecError::Encoding(e.to_string()))?;

        trace!(
            "Encoded {} samples to {} bytes",
            samples.len(),
            encoded_len
        );

        Ok(EncodedFrame::new(
            self.encode_buffer[..encoded_len].to_vec(),
            samples.len(),
        ))
    }

    /// Encode a frame and return the encoded bytes directly.
    pub fn encode_to_bytes(&mut self, samples: &[f32]) -> Result<Vec<u8>, CodecError> {
        self.encode(samples).map(|f| f.data)
    }

    /// Get the expected frame size in samples.
    pub fn frame_size(&self) -> usize {
        self.frame_size
    }

    /// Set the bitrate in bits per second.
    pub fn set_bitrate(&mut self, bitrate: i32) -> Result<(), CodecError> {
        self.encoder
            .set_bitrate(Bitrate::BitsPerSecond(bitrate))
            .map_err(|e| CodecError::EncoderCreation(e.to_string()))
    }

    /// Enable or disable DTX (discontinuous transmission).
    pub fn set_dtx(&mut self, enabled: bool) -> Result<(), CodecError> {
        self.encoder
            .set_dtx(enabled)
            .map_err(|e| CodecError::EncoderCreation(e.to_string()))
    }

    /// Enable or disable FEC (forward error correction).
    pub fn set_fec(&mut self, enabled: bool) -> Result<(), CodecError> {
        self.encoder
            .set_inband_fec(enabled)
            .map_err(|e| CodecError::EncoderCreation(e.to_string()))
    }
}

/// Opus decoder for voice audio.
pub struct OpusDecoder {
    decoder: OpusDecoderInner,
    frame_size: usize,
    decode_buffer: Vec<i16>,
}

impl OpusDecoder {
    /// Create a new Opus decoder with default settings.
    pub fn new() -> Result<Self, CodecError> {
        Self::with_sample_rate(audio_params::SAMPLE_RATE)
    }

    /// Create a new Opus decoder with custom sample rate.
    pub fn with_sample_rate(sample_rate: u32) -> Result<Self, CodecError> {
        let opus_sample_rate = match sample_rate {
            8000 => SampleRate::Hz8000,
            12000 => SampleRate::Hz12000,
            16000 => SampleRate::Hz16000,
            24000 => SampleRate::Hz24000,
            48000 => SampleRate::Hz48000,
            _ => {
                return Err(CodecError::DecoderCreation(format!(
                    "Unsupported sample rate: {}",
                    sample_rate
                )))
            }
        };

        let decoder = OpusDecoderInner::new(opus_sample_rate, Channels::Mono)
            .map_err(|e| CodecError::DecoderCreation(e.to_string()))?;

        debug!("Created Opus decoder: {}Hz, mono", sample_rate);

        Ok(Self {
            decoder,
            frame_size: audio_params::FRAME_SIZE,
            decode_buffer: vec![0i16; audio_params::FRAME_SIZE * 6], // Allow for larger frames
        })
    }

    /// Decode an Opus frame.
    ///
    /// Returns f32 samples normalized to -1.0..1.0.
    pub fn decode(&mut self, data: &[u8]) -> Result<DecodedFrame, CodecError> {
        let packet = Packet::try_from(data)
            .map_err(|e| CodecError::Decoding(e.to_string()))?;

        let output = MutSignals::try_from(&mut self.decode_buffer[..])
            .map_err(|e| CodecError::Decoding(e.to_string()))?;

        let decoded_len = self
            .decoder
            .decode(Some(packet), output, false)
            .map_err(|e| CodecError::Decoding(e.to_string()))?;

        let samples: Vec<f32> = self.decode_buffer[..decoded_len]
            .iter()
            .map(|&s| s as f32 / 32768.0)
            .collect();

        trace!("Decoded {} bytes to {} samples", data.len(), samples.len());

        Ok(DecodedFrame::new(samples))
    }

    /// Decode using FEC (forward error correction) when a packet is lost.
    ///
    /// Pass the next packet's data if available, or None for PLC.
    pub fn decode_fec(&mut self, next_packet: Option<&[u8]>) -> Result<DecodedFrame, CodecError> {
        let packet = match next_packet {
            Some(data) => Some(
                Packet::try_from(data)
                    .map_err(|e| CodecError::Decoding(e.to_string()))?
            ),
            None => None,
        };

        let output = MutSignals::try_from(&mut self.decode_buffer[..])
            .map_err(|e| CodecError::Decoding(e.to_string()))?;

        let decoded_len = self
            .decoder
            .decode(packet, output, true)
            .map_err(|e| CodecError::Decoding(e.to_string()))?;

        let samples: Vec<f32> = self.decode_buffer[..decoded_len]
            .iter()
            .map(|&s| s as f32 / 32768.0)
            .collect();

        trace!("Decoded FEC to {} samples", samples.len());

        Ok(DecodedFrame::fec(samples))
    }

    /// Decode a missing packet using PLC (packet loss concealment).
    pub fn decode_plc(&mut self) -> Result<DecodedFrame, CodecError> {
        let output = MutSignals::try_from(&mut self.decode_buffer[..])
            .map_err(|e| CodecError::Decoding(e.to_string()))?;

        let decoded_len = self
            .decoder
            .decode(None, output, false)
            .map_err(|e| CodecError::Decoding(e.to_string()))?;

        let samples: Vec<f32> = self.decode_buffer[..decoded_len]
            .iter()
            .map(|&s| s as f32 / 32768.0)
            .collect();

        trace!("PLC generated {} samples", samples.len());

        Ok(DecodedFrame::fec(samples))
    }

    /// Get the expected frame size in samples.
    pub fn frame_size(&self) -> usize {
        self.frame_size
    }

    /// Reset the decoder state.
    pub fn reset(&mut self) -> Result<(), CodecError> {
        self.decoder
            .reset_state()
            .map_err(|e| CodecError::DecoderCreation(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoder_creation() {
        let encoder = OpusEncoder::new();
        assert!(encoder.is_ok());
    }

    #[test]
    fn test_decoder_creation() {
        let decoder = OpusDecoder::new();
        assert!(decoder.is_ok());
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let mut encoder = OpusEncoder::new().unwrap();
        let mut decoder = OpusDecoder::new().unwrap();

        // Create a simple sine wave
        let samples: Vec<f32> = (0..480)
            .map(|i| (i as f32 * 0.1).sin() * 0.5)
            .collect();

        // Encode
        let encoded = encoder.encode(&samples).unwrap();
        assert!(!encoded.data.is_empty());
        assert!(encoded.data.len() < samples.len() * 4); // Should be compressed

        // Decode
        let decoded = decoder.decode(&encoded.data).unwrap();
        assert_eq!(decoded.samples.len(), samples.len());

        // Check that decoded audio is similar to original (not exact due to lossy compression)
        let correlation: f32 = samples
            .iter()
            .zip(decoded.samples.iter())
            .map(|(a, b)| a * b)
            .sum();
        assert!(correlation > 0.0, "Audio should be correlated");
    }

    #[test]
    fn test_encode_silence() {
        let mut encoder = OpusEncoder::new().unwrap();
        let mut decoder = OpusDecoder::new().unwrap();

        let silence = vec![0.0f32; 480];

        let encoded = encoder.encode(&silence).unwrap();
        let decoded = decoder.decode(&encoded.data).unwrap();

        // Decoded silence should be near zero
        let max_sample = decoded
            .samples
            .iter()
            .map(|s| s.abs())
            .fold(0.0f32, |a, b| a.max(b));
        assert!(
            max_sample < 0.01,
            "Decoded silence should be quiet: {}",
            max_sample
        );
    }

    #[test]
    fn test_plc() {
        let mut encoder = OpusEncoder::new().unwrap();
        let mut decoder = OpusDecoder::new().unwrap();

        // Encode a few frames to build up decoder state
        for _ in 0..5 {
            let samples: Vec<f32> = (0..480)
                .map(|i| (i as f32 * 0.1).sin() * 0.5)
                .collect();
            let encoded = encoder.encode(&samples).unwrap();
            decoder.decode(&encoded.data).unwrap();
        }

        // Now test PLC
        let plc_frame = decoder.decode_plc().unwrap();
        assert_eq!(plc_frame.samples.len(), 480);
    }

    #[test]
    fn test_invalid_frame_size() {
        let mut encoder = OpusEncoder::new().unwrap();

        // Try to encode wrong size
        let samples = vec![0.0f32; 100];
        let result = encoder.encode(&samples);
        assert!(result.is_err());
    }

    #[test]
    fn test_bitrate_change() {
        let mut encoder = OpusEncoder::new().unwrap();

        // Should succeed
        assert!(encoder.set_bitrate(24000).is_ok());
        assert!(encoder.set_bitrate(64000).is_ok());

        // Encode should still work
        let samples = vec![0.0f32; 480];
        assert!(encoder.encode(&samples).is_ok());
    }
}
