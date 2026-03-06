//! Adaptive jitter buffer for handling network timing variations.

use std::collections::BTreeMap;

use tracing::{debug, trace, warn};

use crate::MixerError;
use intercom_protocol::audio_params;

/// Configuration for the jitter buffer.
#[derive(Debug, Clone)]
pub struct JitterBufferConfig {
    /// Minimum buffer delay in milliseconds.
    pub min_delay_ms: u32,
    /// Maximum buffer delay in milliseconds.
    pub max_delay_ms: u32,
    /// Initial buffer delay in milliseconds.
    pub initial_delay_ms: u32,
    /// Number of samples per frame.
    pub frame_size: usize,
    /// Sample rate in Hz.
    pub sample_rate: u32,
}

impl Default for JitterBufferConfig {
    fn default() -> Self {
        Self {
            min_delay_ms: 20,
            max_delay_ms: 100,
            initial_delay_ms: 40,
            frame_size: audio_params::FRAME_SIZE,
            sample_rate: audio_params::SAMPLE_RATE,
        }
    }
}

/// A packet stored in the jitter buffer.
#[derive(Debug, Clone)]
pub struct BufferedPacket {
    /// Sequence number.
    pub sequence: u32,
    /// Timestamp in samples.
    pub timestamp: u64,
    /// Audio samples.
    pub samples: Vec<f32>,
    /// Source user ID.
    pub user_id: String,
    /// Arrival time (for jitter calculation).
    arrival_time: std::time::Instant,
}

/// Adaptive jitter buffer for a single audio stream.
pub struct JitterBuffer {
    config: JitterBufferConfig,
    /// Packets ordered by sequence number.
    packets: BTreeMap<u32, BufferedPacket>,
    /// Next sequence number to output.
    next_sequence: Option<u32>,
    /// Current buffer delay in frames.
    target_delay_frames: u32,
    /// Statistics for adaptive delay.
    jitter_estimate_ms: f32,
    /// Last packet arrival time.
    last_arrival: Option<std::time::Instant>,
    /// Last packet timestamp.
    last_timestamp: Option<u64>,
    /// Number of consecutive underruns.
    underrun_count: u32,
    /// Number of packets received.
    packets_received: u64,
    /// Number of packets lost.
    packets_lost: u64,
}

impl JitterBuffer {
    /// Create a new jitter buffer with default configuration.
    pub fn new() -> Self {
        Self::with_config(JitterBufferConfig::default())
    }

    /// Create a new jitter buffer with custom configuration.
    pub fn with_config(config: JitterBufferConfig) -> Self {
        let initial_delay_frames =
            (config.initial_delay_ms * config.sample_rate / 1000) / config.frame_size as u32;

        Self {
            config,
            packets: BTreeMap::new(),
            next_sequence: None,
            target_delay_frames: initial_delay_frames,
            jitter_estimate_ms: 0.0,
            last_arrival: None,
            last_timestamp: None,
            underrun_count: 0,
            packets_received: 0,
            packets_lost: 0,
        }
    }

    /// Push a packet into the buffer.
    pub fn push(&mut self, sequence: u32, timestamp: u64, samples: Vec<f32>, user_id: String) {
        let now = std::time::Instant::now();
        self.packets_received += 1;

        // Update jitter estimate
        if let (Some(last_arrival), Some(last_ts)) = (self.last_arrival, self.last_timestamp) {
            let arrival_delta_ms = last_arrival.elapsed().as_secs_f32() * 1000.0;
            let expected_delta_ms =
                (timestamp.saturating_sub(last_ts) as f32 / self.config.sample_rate as f32) * 1000.0;
            let jitter = (arrival_delta_ms - expected_delta_ms).abs();

            // Exponential moving average
            self.jitter_estimate_ms = self.jitter_estimate_ms * 0.9 + jitter * 0.1;
        }

        self.last_arrival = Some(now);
        self.last_timestamp = Some(timestamp);

        // Initialize next_sequence on first packet
        if self.next_sequence.is_none() {
            self.next_sequence = Some(sequence);
        }

        // Check for old packet (already played or too old)
        if let Some(next) = self.next_sequence {
            let diff = sequence.wrapping_sub(next) as i32;
            if diff < -100 {
                trace!("Dropping old packet: seq {} (expecting {})", sequence, next);
                return;
            }
        }

        let packet = BufferedPacket {
            sequence,
            timestamp,
            samples,
            user_id,
            arrival_time: now,
        };

        self.packets.insert(sequence, packet);

        // Limit buffer size
        while self.packets.len() > 100 {
            if let Some((&oldest_seq, _)) = self.packets.first_key_value() {
                self.packets.remove(&oldest_seq);
            }
        }

        // Adapt delay based on jitter
        self.adapt_delay();
    }

    /// Pop the next frame from the buffer.
    ///
    /// Returns the audio samples if available, or None if buffer is empty/waiting.
    pub fn pop(&mut self) -> Option<BufferedPacket> {
        let next_seq = self.next_sequence?;

        // Check if we have enough buffered packets
        let buffered_frames = self.packets.len() as u32;
        if buffered_frames < self.target_delay_frames && self.underrun_count < 3 {
            // Still filling buffer
            trace!(
                "Buffer filling: {}/{} frames",
                buffered_frames,
                self.target_delay_frames
            );
            return None;
        }

        if let Some(packet) = self.packets.remove(&next_seq) {
            self.next_sequence = Some(next_seq.wrapping_add(1));
            self.underrun_count = 0;
            Some(packet)
        } else {
            // Packet missing - check if we should skip ahead
            self.underrun_count += 1;
            self.packets_lost += 1;

            if self.underrun_count > 3 {
                // Skip to next available packet
                if let Some(&first_seq) = self.packets.keys().next() {
                    let skipped = first_seq.wrapping_sub(next_seq);
                    warn!("Skipping {} missing packets", skipped);
                    self.next_sequence = Some(first_seq);
                    self.underrun_count = 0;
                    return self.packets.remove(&first_seq);
                }
            }

            trace!("Missing packet: seq {}", next_seq);
            self.next_sequence = Some(next_seq.wrapping_add(1));
            None
        }
    }

    /// Get a frame, generating silence if nothing is available.
    pub fn pop_or_silence(&mut self) -> Vec<f32> {
        self.pop()
            .map(|p| p.samples)
            .unwrap_or_else(|| vec![0.0; self.config.frame_size])
    }

    /// Adapt the target delay based on observed jitter.
    fn adapt_delay(&mut self) {
        // Target delay = 2 * jitter estimate, clamped to min/max
        let jitter_frames = (self.jitter_estimate_ms * self.config.sample_rate as f32 / 1000.0)
            / self.config.frame_size as f32;
        let target = (jitter_frames * 2.0).ceil() as u32;

        let min_frames =
            (self.config.min_delay_ms * self.config.sample_rate / 1000) / self.config.frame_size as u32;
        let max_frames =
            (self.config.max_delay_ms * self.config.sample_rate / 1000) / self.config.frame_size as u32;

        let new_target = target.clamp(min_frames, max_frames);

        if new_target != self.target_delay_frames {
            debug!(
                "Adjusting jitter buffer: {} -> {} frames (jitter: {:.1}ms)",
                self.target_delay_frames, new_target, self.jitter_estimate_ms
            );
            self.target_delay_frames = new_target;
        }
    }

    /// Get the current buffer delay in milliseconds.
    pub fn current_delay_ms(&self) -> f32 {
        let frames = self.packets.len() as f32;
        (frames * self.config.frame_size as f32 / self.config.sample_rate as f32) * 1000.0
    }

    /// Get the target buffer delay in milliseconds.
    pub fn target_delay_ms(&self) -> f32 {
        (self.target_delay_frames as f32 * self.config.frame_size as f32
            / self.config.sample_rate as f32)
            * 1000.0
    }

    /// Get the estimated jitter in milliseconds.
    pub fn jitter_ms(&self) -> f32 {
        self.jitter_estimate_ms
    }

    /// Get packet loss statistics.
    pub fn packet_loss_rate(&self) -> f32 {
        if self.packets_received == 0 {
            return 0.0;
        }
        self.packets_lost as f32 / (self.packets_received + self.packets_lost) as f32
    }

    /// Get the number of buffered packets.
    pub fn buffered_count(&self) -> usize {
        self.packets.len()
    }

    /// Reset the buffer state.
    pub fn reset(&mut self) {
        self.packets.clear();
        self.next_sequence = None;
        self.underrun_count = 0;
        self.jitter_estimate_ms = 0.0;
        self.last_arrival = None;
        self.last_timestamp = None;
        let initial_delay_frames = (self.config.initial_delay_ms * self.config.sample_rate / 1000)
            / self.config.frame_size as u32;
        self.target_delay_frames = initial_delay_frames;
    }
}

impl Default for JitterBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_push_pop() {
        let mut jb = JitterBuffer::with_config(JitterBufferConfig {
            min_delay_ms: 0,
            initial_delay_ms: 0,
            ..Default::default()
        });

        // Push some packets
        for i in 0..10u32 {
            let samples = vec![i as f32; 480];
            jb.push(i, i as u64 * 480, samples, "user1".to_string());
        }

        // Pop them back
        for i in 0..10u32 {
            let packet = jb.pop().expect("Should have packet");
            assert_eq!(packet.sequence, i);
        }
    }

    #[test]
    fn test_out_of_order() {
        let mut jb = JitterBuffer::with_config(JitterBufferConfig {
            min_delay_ms: 0,
            initial_delay_ms: 0,
            ..Default::default()
        });

        // Push packets out of order
        jb.push(2, 960, vec![2.0; 480], "user1".to_string());
        jb.push(0, 0, vec![0.0; 480], "user1".to_string());
        jb.push(1, 480, vec![1.0; 480], "user1".to_string());

        // Should get them in order
        assert_eq!(jb.pop().unwrap().sequence, 0);
        assert_eq!(jb.pop().unwrap().sequence, 1);
        assert_eq!(jb.pop().unwrap().sequence, 2);
    }

    #[test]
    fn test_missing_packet() {
        let mut jb = JitterBuffer::with_config(JitterBufferConfig {
            min_delay_ms: 0,
            initial_delay_ms: 0,
            ..Default::default()
        });

        // Push packets with gap
        jb.push(0, 0, vec![0.0; 480], "user1".to_string());
        jb.push(2, 960, vec![2.0; 480], "user1".to_string());
        jb.push(3, 1440, vec![3.0; 480], "user1".to_string());

        // Pop first packet
        assert_eq!(jb.pop().unwrap().sequence, 0);

        // Missing packet 1 - should return None initially
        assert!(jb.pop().is_none());

        // After several attempts, should skip to next available
        for _ in 0..5 {
            let _ = jb.pop();
        }
    }

    #[test]
    fn test_pop_or_silence() {
        let mut jb = JitterBuffer::with_config(JitterBufferConfig {
            min_delay_ms: 0,
            initial_delay_ms: 0,
            ..Default::default()
        });

        // Empty buffer should return silence
        let silence = jb.pop_or_silence();
        assert_eq!(silence.len(), 480);
        assert!(silence.iter().all(|&s| s == 0.0));

        // Push a packet
        jb.push(0, 0, vec![1.0; 480], "user1".to_string());
        let samples = jb.pop_or_silence();
        assert!(samples.iter().all(|&s| s == 1.0));
    }

    #[test]
    fn test_reset() {
        let mut jb = JitterBuffer::new();

        // Add some packets
        for i in 0..5u32 {
            jb.push(i, i as u64 * 480, vec![i as f32; 480], "user1".to_string());
        }

        assert!(jb.buffered_count() > 0);

        jb.reset();

        assert_eq!(jb.buffered_count(), 0);
        assert!(jb.pop().is_none());
    }
}
