//! Audio I/O module for the intercom system.
//!
//! This crate provides audio capture and playback functionality using cpal.

pub mod cpal_backend;
pub mod device;
pub mod handle;
pub mod resampler;
pub mod vad;

pub use cpal_backend::{AudioCapture, AudioPlayback};
pub use device::{AudioDevice, DeviceType};
pub use handle::AudioHandle;

use thiserror::Error;

/// Audio errors.
#[derive(Debug, Error)]
pub enum AudioError {
    #[error("No audio device available")]
    NoDevice,

    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    #[error("Failed to get device config: {0}")]
    ConfigError(String),

    #[error("Failed to build audio stream: {0}")]
    StreamError(String),

    #[error("Stream playback error: {0}")]
    PlaybackError(String),

    #[error("Audio channel closed")]
    ChannelClosed,

    #[error("Sample rate mismatch: expected {expected}, got {actual}")]
    SampleRateMismatch { expected: u32, actual: u32 },

    #[error("Unsupported sample format")]
    UnsupportedFormat,
}

/// Audio sample type used internally (f32 normalized to -1.0..1.0).
pub type Sample = f32;

/// A buffer of audio samples.
#[derive(Debug, Clone)]
pub struct AudioBuffer {
    /// The audio samples.
    pub samples: Vec<Sample>,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Number of channels.
    pub channels: u16,
}

impl AudioBuffer {
    /// Create a new audio buffer.
    pub fn new(samples: Vec<Sample>, sample_rate: u32, channels: u16) -> Self {
        Self {
            samples,
            sample_rate,
            channels,
        }
    }

    /// Create an empty buffer with the given parameters.
    pub fn empty(sample_rate: u32, channels: u16) -> Self {
        Self {
            samples: Vec::new(),
            sample_rate,
            channels,
        }
    }

    /// Create a silent buffer with the given number of samples.
    pub fn silence(num_samples: usize, sample_rate: u32, channels: u16) -> Self {
        Self {
            samples: vec![0.0; num_samples],
            sample_rate,
            channels,
        }
    }

    /// Get the duration in milliseconds.
    pub fn duration_ms(&self) -> f64 {
        let frames = self.samples.len() / self.channels as usize;
        (frames as f64 / self.sample_rate as f64) * 1000.0
    }

    /// Get the number of frames (samples per channel).
    pub fn num_frames(&self) -> usize {
        self.samples.len() / self.channels as usize
    }

    /// Convert to mono by averaging channels.
    pub fn to_mono(&self) -> Self {
        if self.channels == 1 {
            return self.clone();
        }

        let frames = self.num_frames();
        let mut mono_samples = Vec::with_capacity(frames);

        for frame_idx in 0..frames {
            let mut sum = 0.0;
            for ch in 0..self.channels as usize {
                sum += self.samples[frame_idx * self.channels as usize + ch];
            }
            mono_samples.push(sum / self.channels as f32);
        }

        Self {
            samples: mono_samples,
            sample_rate: self.sample_rate,
            channels: 1,
        }
    }

    /// Apply gain to the buffer.
    pub fn apply_gain(&mut self, gain: f32) {
        for sample in &mut self.samples {
            *sample *= gain;
        }
    }

    /// Mix another buffer into this one.
    pub fn mix(&mut self, other: &AudioBuffer) {
        let len = self.samples.len().min(other.samples.len());
        for i in 0..len {
            self.samples[i] += other.samples[i];
        }
    }

    /// Clamp all samples to valid range.
    pub fn clamp(&mut self) {
        for sample in &mut self.samples {
            *sample = sample.clamp(-1.0, 1.0);
        }
    }
}

/// Callback type for receiving captured audio.
pub type CaptureCallback = Box<dyn FnMut(AudioBuffer) + Send + 'static>;

/// Callback type for providing playback audio.
pub type PlaybackCallback = Box<dyn FnMut(usize) -> AudioBuffer + Send + 'static>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_buffer_mono_conversion() {
        // Stereo buffer: L=1.0, R=0.0, L=0.5, R=0.5
        let stereo = AudioBuffer::new(vec![1.0, 0.0, 0.5, 0.5], 48000, 2);
        let mono = stereo.to_mono();

        assert_eq!(mono.channels, 1);
        assert_eq!(mono.samples.len(), 2);
        assert_eq!(mono.samples[0], 0.5); // avg(1.0, 0.0)
        assert_eq!(mono.samples[1], 0.5); // avg(0.5, 0.5)
    }

    #[test]
    fn test_audio_buffer_duration() {
        let buffer = AudioBuffer::new(vec![0.0; 480], 48000, 1);
        let duration = buffer.duration_ms();
        assert!((duration - 10.0).abs() < 0.001); // 480 samples at 48kHz = 10ms
    }

    #[test]
    fn test_audio_buffer_mix() {
        let mut a = AudioBuffer::new(vec![0.5, 0.5], 48000, 1);
        let b = AudioBuffer::new(vec![0.3, 0.4], 48000, 1);
        a.mix(&b);

        assert!((a.samples[0] - 0.8).abs() < 0.001);
        assert!((a.samples[1] - 0.9).abs() < 0.001);
    }
}
