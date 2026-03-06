//! Channel mixer for combining multiple audio streams.

use std::collections::HashMap;

use parking_lot::RwLock;
use tracing::{debug, trace};

use crate::jitter_buffer::{JitterBuffer, JitterBufferConfig};
use crate::{MixedAudio, MixerError};
use intercom_protocol::{ChannelId, UserId};

/// Per-user audio stream state.
struct UserStream {
    user_id: UserId,
    jitter_buffer: JitterBuffer,
    gain: f32,
    muted: bool,
    last_activity: std::time::Instant,
}

impl UserStream {
    fn new(user_id: UserId, config: JitterBufferConfig) -> Self {
        Self {
            user_id,
            jitter_buffer: JitterBuffer::with_config(config),
            gain: 1.0,
            muted: false,
            last_activity: std::time::Instant::now(),
        }
    }
}

/// A channel that can have multiple users talking.
struct Channel {
    id: ChannelId,
    users: HashMap<String, UserStream>,
    master_gain: f32,
}

impl Channel {
    fn new(id: ChannelId) -> Self {
        Self {
            id,
            users: HashMap::new(),
            master_gain: 1.0,
        }
    }
}

/// Audio mixer for combining multiple channels and users.
pub struct ChannelMixer {
    channels: RwLock<HashMap<String, Channel>>,
    jitter_config: JitterBufferConfig,
    frame_size: usize,
    master_gain: f32,
}

impl ChannelMixer {
    /// Create a new channel mixer.
    pub fn new() -> Self {
        Self::with_config(JitterBufferConfig::default())
    }

    /// Create a new channel mixer with custom jitter buffer config.
    pub fn with_config(jitter_config: JitterBufferConfig) -> Self {
        let frame_size = jitter_config.frame_size;
        Self {
            channels: RwLock::new(HashMap::new()),
            jitter_config,
            frame_size,
            master_gain: 1.0,
        }
    }

    /// Add a channel.
    pub fn add_channel(&self, channel_id: &ChannelId) {
        let mut channels = self.channels.write();
        if !channels.contains_key(channel_id.as_str()) {
            channels.insert(channel_id.0.clone(), Channel::new(channel_id.clone()));
            debug!("Added channel: {}", channel_id);
        }
    }

    /// Remove a channel.
    pub fn remove_channel(&self, channel_id: &ChannelId) {
        let mut channels = self.channels.write();
        channels.remove(channel_id.as_str());
        debug!("Removed channel: {}", channel_id);
    }

    /// Add a user to a channel.
    pub fn add_user(&self, channel_id: &ChannelId, user_id: &UserId) {
        let mut channels = self.channels.write();
        if let Some(channel) = channels.get_mut(channel_id.as_str()) {
            if !channel.users.contains_key(user_id.as_str()) {
                channel.users.insert(
                    user_id.0.clone(),
                    UserStream::new(user_id.clone(), self.jitter_config.clone()),
                );
                debug!("Added user {} to channel {}", user_id, channel_id);
            }
        }
    }

    /// Remove a user from a channel.
    pub fn remove_user(&self, channel_id: &ChannelId, user_id: &UserId) {
        let mut channels = self.channels.write();
        if let Some(channel) = channels.get_mut(channel_id.as_str()) {
            channel.users.remove(user_id.as_str());
            debug!("Removed user {} from channel {}", user_id, channel_id);
        }
    }

    /// Push audio data for a user on a channel.
    pub fn push_audio(
        &self,
        channel_id: &ChannelId,
        user_id: &UserId,
        sequence: u32,
        timestamp: u64,
        samples: Vec<f32>,
    ) -> Result<(), MixerError> {
        let mut channels = self.channels.write();
        let channel = channels
            .get_mut(channel_id.as_str())
            .ok_or_else(|| MixerError::ChannelNotFound(channel_id.to_string()))?;

        // Auto-add user if not present
        if !channel.users.contains_key(user_id.as_str()) {
            channel.users.insert(
                user_id.0.clone(),
                UserStream::new(user_id.clone(), self.jitter_config.clone()),
            );
        }

        let user_stream = channel
            .users
            .get_mut(user_id.as_str())
            .ok_or_else(|| MixerError::UserNotFound(user_id.to_string()))?;

        user_stream
            .jitter_buffer
            .push(sequence, timestamp, samples, user_id.to_string());
        user_stream.last_activity = std::time::Instant::now();

        Ok(())
    }

    /// Get mixed audio for a specific channel.
    pub fn mix_channel(&self, channel_id: &ChannelId) -> Result<MixedAudio, MixerError> {
        let mut channels = self.channels.write();
        let channel = channels
            .get_mut(channel_id.as_str())
            .ok_or_else(|| MixerError::ChannelNotFound(channel_id.to_string()))?;

        let mut mixed = vec![0.0f32; self.frame_size];
        let mut speakers = Vec::new();
        let mut has_audio = false;

        for (user_id, stream) in channel.users.iter_mut() {
            if stream.muted {
                continue;
            }

            if let Some(packet) = stream.jitter_buffer.pop() {
                has_audio = true;
                speakers.push(user_id.clone());

                let gain = stream.gain * channel.master_gain * self.master_gain;
                for (i, &sample) in packet.samples.iter().enumerate() {
                    if i < mixed.len() {
                        mixed[i] += sample * gain;
                    }
                }
            }
        }

        // Soft clipping
        if has_audio {
            for sample in &mut mixed {
                *sample = soft_clip(*sample);
            }
        }

        Ok(MixedAudio {
            samples: mixed,
            speakers,
            timestamp: 0, // Would need to track this
        })
    }

    /// Get mixed audio for multiple channels.
    pub fn mix_channels(&self, channel_ids: &[ChannelId]) -> Result<MixedAudio, MixerError> {
        let mut total_mixed = vec![0.0f32; self.frame_size];
        let mut all_speakers = Vec::new();

        for channel_id in channel_ids {
            let channel_mix = self.mix_channel(channel_id)?;
            all_speakers.extend(channel_mix.speakers);

            for (i, &sample) in channel_mix.samples.iter().enumerate() {
                if i < total_mixed.len() {
                    total_mixed[i] += sample;
                }
            }
        }

        // Final soft clipping
        for sample in &mut total_mixed {
            *sample = soft_clip(*sample);
        }

        Ok(MixedAudio {
            samples: total_mixed,
            speakers: all_speakers,
            timestamp: 0,
        })
    }

    /// Set user gain (0.0 - 2.0 typical range).
    pub fn set_user_gain(
        &self,
        channel_id: &ChannelId,
        user_id: &UserId,
        gain: f32,
    ) -> Result<(), MixerError> {
        let mut channels = self.channels.write();
        let channel = channels
            .get_mut(channel_id.as_str())
            .ok_or_else(|| MixerError::ChannelNotFound(channel_id.to_string()))?;

        let stream = channel
            .users
            .get_mut(user_id.as_str())
            .ok_or_else(|| MixerError::UserNotFound(user_id.to_string()))?;

        stream.gain = gain.max(0.0);
        Ok(())
    }

    /// Mute/unmute a user.
    pub fn set_user_muted(
        &self,
        channel_id: &ChannelId,
        user_id: &UserId,
        muted: bool,
    ) -> Result<(), MixerError> {
        let mut channels = self.channels.write();
        let channel = channels
            .get_mut(channel_id.as_str())
            .ok_or_else(|| MixerError::ChannelNotFound(channel_id.to_string()))?;

        let stream = channel
            .users
            .get_mut(user_id.as_str())
            .ok_or_else(|| MixerError::UserNotFound(user_id.to_string()))?;

        stream.muted = muted;
        Ok(())
    }

    /// Set channel master gain.
    pub fn set_channel_gain(&self, channel_id: &ChannelId, gain: f32) -> Result<(), MixerError> {
        let mut channels = self.channels.write();
        let channel = channels
            .get_mut(channel_id.as_str())
            .ok_or_else(|| MixerError::ChannelNotFound(channel_id.to_string()))?;

        channel.master_gain = gain.max(0.0);
        Ok(())
    }

    /// Set global master gain.
    pub fn set_master_gain(&mut self, gain: f32) {
        self.master_gain = gain.max(0.0);
    }

    /// Get buffer statistics for a user.
    pub fn get_user_stats(
        &self,
        channel_id: &ChannelId,
        user_id: &UserId,
    ) -> Option<BufferStats> {
        let channels = self.channels.read();
        let channel = channels.get(channel_id.as_str())?;
        let stream = channel.users.get(user_id.as_str())?;

        Some(BufferStats {
            buffered_frames: stream.jitter_buffer.buffered_count(),
            delay_ms: stream.jitter_buffer.current_delay_ms(),
            target_delay_ms: stream.jitter_buffer.target_delay_ms(),
            jitter_ms: stream.jitter_buffer.jitter_ms(),
            packet_loss_rate: stream.jitter_buffer.packet_loss_rate(),
        })
    }

    /// Clean up inactive users (no audio for specified duration).
    pub fn cleanup_inactive(&self, max_idle_secs: u64) {
        let mut channels = self.channels.write();
        let threshold = std::time::Duration::from_secs(max_idle_secs);

        for channel in channels.values_mut() {
            channel.users.retain(|user_id, stream| {
                let idle = stream.last_activity.elapsed() < threshold;
                if !idle {
                    debug!(
                        "Removing inactive user {} from channel {}",
                        user_id, channel.id
                    );
                }
                idle
            });
        }
    }

    /// Get list of channels.
    pub fn list_channels(&self) -> Vec<ChannelId> {
        let channels = self.channels.read();
        channels.keys().map(|k| ChannelId::new(k.clone())).collect()
    }

    /// Get list of users in a channel.
    pub fn list_users(&self, channel_id: &ChannelId) -> Vec<UserId> {
        let channels = self.channels.read();
        channels
            .get(channel_id.as_str())
            .map(|c| c.users.keys().map(|k| UserId::new(k.clone())).collect())
            .unwrap_or_default()
    }
}

impl Default for ChannelMixer {
    fn default() -> Self {
        Self::new()
    }
}

/// Buffer statistics for a user stream.
#[derive(Debug, Clone)]
pub struct BufferStats {
    pub buffered_frames: usize,
    pub delay_ms: f32,
    pub target_delay_ms: f32,
    pub jitter_ms: f32,
    pub packet_loss_rate: f32,
}

/// Soft clipping function to prevent harsh distortion.
fn soft_clip(x: f32) -> f32 {
    if x.abs() <= 1.0 {
        x
    } else {
        x.signum() * (1.0 - 1.0 / (1.0 + x.abs()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_remove_channel() {
        let mixer = ChannelMixer::new();
        let channel = ChannelId::new("test");

        mixer.add_channel(&channel);
        assert!(mixer.list_channels().contains(&channel));

        mixer.remove_channel(&channel);
        assert!(!mixer.list_channels().contains(&channel));
    }

    #[test]
    fn test_add_remove_user() {
        let mixer = ChannelMixer::new();
        let channel = ChannelId::new("test");
        let user = UserId::new("user1");

        mixer.add_channel(&channel);
        mixer.add_user(&channel, &user);

        assert!(mixer.list_users(&channel).contains(&user));

        mixer.remove_user(&channel, &user);
        assert!(!mixer.list_users(&channel).contains(&user));
    }

    #[test]
    fn test_push_and_mix() {
        let mixer = ChannelMixer::with_config(JitterBufferConfig {
            min_delay_ms: 0,
            initial_delay_ms: 0,
            ..Default::default()
        });

        let channel = ChannelId::new("test");
        let user = UserId::new("user1");

        mixer.add_channel(&channel);
        mixer.add_user(&channel, &user);

        // Push some audio
        let samples = vec![0.5f32; 480];
        mixer
            .push_audio(&channel, &user, 0, 0, samples.clone())
            .unwrap();

        // Mix and check
        let mixed = mixer.mix_channel(&channel).unwrap();
        assert_eq!(mixed.samples.len(), 480);
        assert!(mixed.speakers.contains(&user.to_string()));
    }

    #[test]
    fn test_multi_user_mixing() {
        let mixer = ChannelMixer::with_config(JitterBufferConfig {
            min_delay_ms: 0,
            initial_delay_ms: 0,
            ..Default::default()
        });

        let channel = ChannelId::new("test");
        let user1 = UserId::new("user1");
        let user2 = UserId::new("user2");

        mixer.add_channel(&channel);

        // Push audio from two users
        mixer
            .push_audio(&channel, &user1, 0, 0, vec![0.3f32; 480])
            .unwrap();
        mixer
            .push_audio(&channel, &user2, 0, 0, vec![0.4f32; 480])
            .unwrap();

        // Mix should combine both
        let mixed = mixer.mix_channel(&channel).unwrap();
        assert_eq!(mixed.speakers.len(), 2);

        // Check that audio is mixed (approximately 0.3 + 0.4 = 0.7)
        assert!((mixed.samples[0] - 0.7).abs() < 0.01);
    }

    #[test]
    fn test_soft_clip() {
        // Below threshold: no change
        assert_eq!(soft_clip(0.5), 0.5);
        assert_eq!(soft_clip(-0.5), -0.5);
        assert_eq!(soft_clip(1.0), 1.0);

        // Above threshold: soft clipped
        let clipped = soft_clip(2.0);
        assert!(clipped > 1.0 && clipped < 2.0);

        // Much above: asymptotically approaches limit
        let very_clipped = soft_clip(10.0);
        assert!(very_clipped < 1.0);
    }
}
