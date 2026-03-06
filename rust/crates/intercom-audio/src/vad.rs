//! Simple Voice Activity Detection (VAD).
//!
//! This module provides a basic energy-based VAD implementation.
//! For production, consider using WebRTC VAD or similar.

use crate::AudioBuffer;

/// Voice Activity Detector configuration.
#[derive(Debug, Clone, Copy)]
pub struct VadConfig {
    /// Threshold for voice detection (0.0 - 1.0).
    pub threshold: f32,
    /// Number of consecutive frames needed to trigger voice start.
    pub attack_frames: usize,
    /// Number of consecutive frames needed to trigger voice end.
    pub release_frames: usize,
    /// Minimum energy to consider as voice.
    pub min_energy: f32,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            threshold: 0.01,
            attack_frames: 2,
            release_frames: 10,
            min_energy: 0.0001,
        }
    }
}

/// Voice Activity Detector.
pub struct Vad {
    config: VadConfig,
    voice_active: bool,
    consecutive_voice: usize,
    consecutive_silence: usize,
    energy_history: Vec<f32>,
    adaptive_threshold: f32,
}

impl Vad {
    /// Create a new VAD with the given configuration.
    pub fn new(config: VadConfig) -> Self {
        Self {
            config,
            voice_active: false,
            consecutive_voice: 0,
            consecutive_silence: 0,
            energy_history: Vec::with_capacity(50),
            adaptive_threshold: config.threshold,
        }
    }

    /// Create a new VAD with default configuration.
    pub fn default() -> Self {
        Self::new(VadConfig::default())
    }

    /// Process an audio buffer and return whether voice is detected.
    pub fn process(&mut self, buffer: &AudioBuffer) -> bool {
        let energy = self.calculate_energy(&buffer.samples);

        // Update energy history for adaptive threshold
        self.energy_history.push(energy);
        if self.energy_history.len() > 50 {
            self.energy_history.remove(0);
        }

        // Update adaptive threshold based on noise floor
        self.update_adaptive_threshold();

        let is_voice = energy > self.adaptive_threshold && energy > self.config.min_energy;

        if is_voice {
            self.consecutive_voice += 1;
            self.consecutive_silence = 0;

            if !self.voice_active && self.consecutive_voice >= self.config.attack_frames {
                self.voice_active = true;
            }
        } else {
            self.consecutive_silence += 1;
            self.consecutive_voice = 0;

            if self.voice_active && self.consecutive_silence >= self.config.release_frames {
                self.voice_active = false;
            }
        }

        self.voice_active
    }

    /// Check if voice is currently active.
    pub fn is_voice_active(&self) -> bool {
        self.voice_active
    }

    /// Reset the VAD state.
    pub fn reset(&mut self) {
        self.voice_active = false;
        self.consecutive_voice = 0;
        self.consecutive_silence = 0;
        self.energy_history.clear();
        self.adaptive_threshold = self.config.threshold;
    }

    /// Calculate the RMS energy of the samples.
    fn calculate_energy(&self, samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }

        let sum_squares: f32 = samples.iter().map(|s| s * s).sum();
        (sum_squares / samples.len() as f32).sqrt()
    }

    /// Update the adaptive threshold based on noise floor estimation.
    fn update_adaptive_threshold(&mut self) {
        if self.energy_history.len() < 10 {
            return;
        }

        // Estimate noise floor as the 10th percentile of energy
        let mut sorted = self.energy_history.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let noise_floor = sorted[sorted.len() / 10];

        // Set threshold above noise floor
        self.adaptive_threshold = (noise_floor * 3.0).max(self.config.threshold);
    }

    /// Get the current adaptive threshold.
    pub fn current_threshold(&self) -> f32 {
        self.adaptive_threshold
    }
}

/// Simple voice activity detection on a single buffer.
pub fn detect_voice(samples: &[f32], threshold: f32) -> bool {
    if samples.is_empty() {
        return false;
    }

    let energy: f32 = samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32;
    energy.sqrt() > threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vad_silence() {
        let mut vad = Vad::default();
        let silence = AudioBuffer::silence(480, 48000, 1);

        // Process several frames of silence
        for _ in 0..20 {
            let result = vad.process(&silence);
            assert!(!result, "Should not detect voice in silence");
        }
    }

    #[test]
    fn test_vad_voice() {
        let mut vad = Vad::default();

        // Create a loud signal
        let samples: Vec<f32> = (0..480).map(|i| (i as f32 * 0.1).sin() * 0.5).collect();
        let voice = AudioBuffer::new(samples, 48000, 1);

        // Process several frames - should eventually detect voice
        let mut detected = false;
        for _ in 0..10 {
            if vad.process(&voice) {
                detected = true;
                break;
            }
        }
        assert!(detected, "Should detect voice in loud signal");
    }

    #[test]
    fn test_vad_transition() {
        let mut vad = Vad::new(VadConfig {
            threshold: 0.01,
            attack_frames: 2,
            release_frames: 3,
            min_energy: 0.0001,
        });

        // Start with silence
        let silence = AudioBuffer::silence(480, 48000, 1);
        for _ in 0..5 {
            vad.process(&silence);
        }
        assert!(!vad.is_voice_active());

        // Switch to voice
        let samples: Vec<f32> = (0..480).map(|i| (i as f32 * 0.1).sin() * 0.5).collect();
        let voice = AudioBuffer::new(samples, 48000, 1);
        for _ in 0..5 {
            vad.process(&voice);
        }
        assert!(vad.is_voice_active());

        // Switch back to silence
        for _ in 0..5 {
            vad.process(&silence);
        }
        assert!(!vad.is_voice_active());
    }

    #[test]
    fn test_detect_voice_simple() {
        let silence = vec![0.0f32; 480];
        assert!(!detect_voice(&silence, 0.01));

        let voice: Vec<f32> = (0..480).map(|i| (i as f32 * 0.1).sin() * 0.5).collect();
        assert!(detect_voice(&voice, 0.01));
    }
}
