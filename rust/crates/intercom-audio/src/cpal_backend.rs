//! cpal-based audio capture and playback.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use crossbeam_channel::{bounded, Receiver, Sender};
use parking_lot::Mutex;
use tracing::{debug, error, info, trace, warn};

use crate::device::{get_cpal_device, get_default_device, DeviceType};
use crate::{AudioBuffer, AudioError, Sample};
use intercom_protocol::audio_params;

/// Audio capture stream.
pub struct AudioCapture {
    stream: cpal::Stream,
    receiver: Receiver<AudioBuffer>,
    running: Arc<AtomicBool>,
    device_name: String,
    config: StreamConfig,
}

impl AudioCapture {
    /// Create a new audio capture stream with the default input device.
    pub fn new() -> Result<Self, AudioError> {
        let device = get_default_device(DeviceType::Input)?;
        let name = device.name().unwrap_or_default();
        Self::with_device(&name)
    }

    /// Create a new audio capture stream with a specific device.
    pub fn with_device(device_name: &str) -> Result<Self, AudioError> {
        let device = get_cpal_device(device_name, DeviceType::Input)?;
        let supported_config = device
            .default_input_config()
            .map_err(|e| AudioError::ConfigError(e.to_string()))?;

        info!(
            "Input device: {}, format: {:?}, channels: {}, sample_rate: {}",
            device_name,
            supported_config.sample_format(),
            supported_config.channels(),
            supported_config.sample_rate().0
        );

        let config: StreamConfig = supported_config.clone().into();
        let sample_format = supported_config.sample_format();

        // Channel for sending audio buffers
        let (sender, receiver) = bounded::<AudioBuffer>(32);
        let running = Arc::new(AtomicBool::new(false));
        let running_clone = running.clone();

        let channels = config.channels;
        let sample_rate = config.sample_rate.0;
        let target_sample_rate = audio_params::SAMPLE_RATE;
        let target_frame_size = audio_params::FRAME_SIZE;

        // Buffer to accumulate samples until we have a full frame
        let frame_buffer = Arc::new(Mutex::new(Vec::with_capacity(
            target_frame_size * channels as usize * 2,
        )));
        let frame_buffer_clone = frame_buffer.clone();

        let err_fn = |err| error!("Audio capture error: {}", err);

        let stream = match sample_format {
            SampleFormat::F32 => device.build_input_stream(
                &config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if !running_clone.load(Ordering::Relaxed) {
                        return;
                    }
                    Self::process_input(
                        data,
                        sample_rate,
                        channels,
                        target_sample_rate,
                        target_frame_size,
                        &sender,
                        &frame_buffer,
                    );
                },
                err_fn,
                None,
            ),
            SampleFormat::I16 => device.build_input_stream(
                &config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    if !running_clone.load(Ordering::Relaxed) {
                        return;
                    }
                    let float_data: Vec<f32> =
                        data.iter().map(|&s| s as f32 / 32768.0).collect();
                    Self::process_input(
                        &float_data,
                        sample_rate,
                        channels,
                        target_sample_rate,
                        target_frame_size,
                        &sender,
                        &frame_buffer_clone,
                    );
                },
                err_fn,
                None,
            ),
            SampleFormat::U16 => device.build_input_stream(
                &config,
                move |data: &[u16], _: &cpal::InputCallbackInfo| {
                    if !running_clone.load(Ordering::Relaxed) {
                        return;
                    }
                    let float_data: Vec<f32> = data
                        .iter()
                        .map(|&s| (s as f32 - 32768.0) / 32768.0)
                        .collect();
                    Self::process_input(
                        &float_data,
                        sample_rate,
                        channels,
                        target_sample_rate,
                        target_frame_size,
                        &sender,
                        &frame_buffer_clone,
                    );
                },
                err_fn,
                None,
            ),
            _ => return Err(AudioError::UnsupportedFormat),
        }
        .map_err(|e| AudioError::StreamError(e.to_string()))?;

        Ok(Self {
            stream,
            receiver,
            running: Arc::new(AtomicBool::new(false)),
            device_name: device_name.to_string(),
            config,
        })
    }

    fn process_input(
        data: &[f32],
        input_sample_rate: u32,
        input_channels: u16,
        target_sample_rate: u32,
        target_frame_size: usize,
        sender: &Sender<AudioBuffer>,
        frame_buffer: &Arc<Mutex<Vec<f32>>>,
    ) {
        let mut buffer = frame_buffer.lock();

        // Convert to mono if needed
        let mono_data: Vec<f32> = if input_channels == 1 {
            data.to_vec()
        } else {
            let num_frames = data.len() / input_channels as usize;
            (0..num_frames)
                .map(|i| {
                    let start = i * input_channels as usize;
                    let sum: f32 = data[start..start + input_channels as usize].iter().sum();
                    sum / input_channels as f32
                })
                .collect()
        };

        // Resample if needed
        let resampled = if input_sample_rate != target_sample_rate {
            crate::resampler::resample(&mono_data, input_sample_rate, target_sample_rate)
        } else {
            mono_data
        };

        buffer.extend(resampled);

        // Send complete frames
        while buffer.len() >= target_frame_size {
            let frame: Vec<f32> = buffer.drain(..target_frame_size).collect();
            let audio_buffer = AudioBuffer::new(frame, target_sample_rate, 1);

            if sender.try_send(audio_buffer).is_err() {
                trace!("Audio buffer channel full, dropping frame");
            }
        }
    }

    /// Start capturing audio.
    pub fn start(&self) -> Result<(), AudioError> {
        self.running.store(true, Ordering::Relaxed);
        self.stream
            .play()
            .map_err(|e| AudioError::StreamError(e.to_string()))?;
        info!("Audio capture started on {}", self.device_name);
        Ok(())
    }

    /// Stop capturing audio.
    pub fn stop(&self) -> Result<(), AudioError> {
        self.running.store(false, Ordering::Relaxed);
        self.stream
            .pause()
            .map_err(|e| AudioError::StreamError(e.to_string()))?;
        info!("Audio capture stopped");
        Ok(())
    }

    /// Receive the next audio buffer (blocking).
    pub fn recv(&self) -> Result<AudioBuffer, AudioError> {
        self.receiver.recv().map_err(|_| AudioError::ChannelClosed)
    }

    /// Try to receive an audio buffer (non-blocking).
    pub fn try_recv(&self) -> Option<AudioBuffer> {
        self.receiver.try_recv().ok()
    }

    /// Get the receiver for async usage.
    pub fn receiver(&self) -> &Receiver<AudioBuffer> {
        &self.receiver
    }

    /// Check if the capture is running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Get the device name.
    pub fn device_name(&self) -> &str {
        &self.device_name
    }
}

/// Audio playback stream.
pub struct AudioPlayback {
    stream: cpal::Stream,
    sender: Sender<AudioBuffer>,
    running: Arc<AtomicBool>,
    device_name: String,
    config: StreamConfig,
}

impl AudioPlayback {
    /// Create a new audio playback stream with the default output device.
    pub fn new() -> Result<Self, AudioError> {
        let device = get_default_device(DeviceType::Output)?;
        let name = device.name().unwrap_or_default();
        Self::with_device(&name)
    }

    /// Create a new audio playback stream with a specific device.
    pub fn with_device(device_name: &str) -> Result<Self, AudioError> {
        let device = get_cpal_device(device_name, DeviceType::Output)?;
        let supported_config = device
            .default_output_config()
            .map_err(|e| AudioError::ConfigError(e.to_string()))?;

        info!(
            "Output device: {}, format: {:?}, channels: {}, sample_rate: {}",
            device_name,
            supported_config.sample_format(),
            supported_config.channels(),
            supported_config.sample_rate().0
        );

        let config: StreamConfig = supported_config.clone().into();
        let sample_format = supported_config.sample_format();

        // Channel for receiving audio buffers
        let (sender, receiver) = bounded::<AudioBuffer>(32);
        let running = Arc::new(AtomicBool::new(false));
        let running_clone = running.clone();

        let channels = config.channels;
        let sample_rate = config.sample_rate.0;
        let source_sample_rate = audio_params::SAMPLE_RATE;

        // Playback buffer for sample-accurate output
        let playback_buffer = Arc::new(Mutex::new(Vec::<f32>::with_capacity(4096)));
        let playback_buffer_clone = playback_buffer.clone();

        let err_fn = |err| error!("Audio playback error: {}", err);

        let stream = match sample_format {
            SampleFormat::F32 => device.build_output_stream(
                &config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    if !running_clone.load(Ordering::Relaxed) {
                        data.fill(0.0);
                        return;
                    }
                    Self::fill_output(
                        data,
                        &receiver,
                        &playback_buffer,
                        source_sample_rate,
                        sample_rate,
                        channels,
                    );
                },
                err_fn,
                None,
            ),
            SampleFormat::I16 => device.build_output_stream(
                &config,
                move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                    if !running_clone.load(Ordering::Relaxed) {
                        data.fill(0);
                        return;
                    }
                    let mut float_data = vec![0.0f32; data.len()];
                    Self::fill_output(
                        &mut float_data,
                        &receiver,
                        &playback_buffer_clone,
                        source_sample_rate,
                        sample_rate,
                        channels,
                    );
                    for (out, &sample) in data.iter_mut().zip(float_data.iter()) {
                        *out = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
                    }
                },
                err_fn,
                None,
            ),
            SampleFormat::U16 => device.build_output_stream(
                &config,
                move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                    if !running_clone.load(Ordering::Relaxed) {
                        data.fill(32768);
                        return;
                    }
                    let mut float_data = vec![0.0f32; data.len()];
                    Self::fill_output(
                        &mut float_data,
                        &receiver,
                        &playback_buffer_clone,
                        source_sample_rate,
                        sample_rate,
                        channels,
                    );
                    for (out, &sample) in data.iter_mut().zip(float_data.iter()) {
                        *out = ((sample * 32767.0) + 32768.0).clamp(0.0, 65535.0) as u16;
                    }
                },
                err_fn,
                None,
            ),
            _ => return Err(AudioError::UnsupportedFormat),
        }
        .map_err(|e| AudioError::StreamError(e.to_string()))?;

        Ok(Self {
            stream,
            sender,
            running: Arc::new(AtomicBool::new(false)),
            device_name: device_name.to_string(),
            config,
        })
    }

    fn fill_output(
        data: &mut [f32],
        receiver: &Receiver<AudioBuffer>,
        buffer: &Arc<Mutex<Vec<f32>>>,
        source_sample_rate: u32,
        output_sample_rate: u32,
        output_channels: u16,
    ) {
        let mut buf = buffer.lock();

        // Receive new audio data
        while let Ok(audio) = receiver.try_recv() {
            // Resample if needed
            let resampled = if source_sample_rate != output_sample_rate {
                crate::resampler::resample(&audio.samples, source_sample_rate, output_sample_rate)
            } else {
                audio.samples
            };

            // Upmix to output channels if needed
            if output_channels == 1 {
                buf.extend(resampled);
            } else {
                for sample in resampled {
                    for _ in 0..output_channels {
                        buf.push(sample);
                    }
                }
            }
        }

        // Fill output buffer
        let needed = data.len();
        if buf.len() >= needed {
            let samples: Vec<f32> = buf.drain(..needed).collect();
            data.copy_from_slice(&samples);
        } else {
            // Not enough data, output what we have and pad with silence
            let available = buf.len();
            if available > 0 {
                let samples: Vec<f32> = buf.drain(..).collect();
                data[..available].copy_from_slice(&samples);
            }
            data[available..].fill(0.0);
        }
    }

    /// Start playing audio.
    pub fn start(&self) -> Result<(), AudioError> {
        self.running.store(true, Ordering::Relaxed);
        self.stream
            .play()
            .map_err(|e| AudioError::StreamError(e.to_string()))?;
        info!("Audio playback started on {}", self.device_name);
        Ok(())
    }

    /// Stop playing audio.
    pub fn stop(&self) -> Result<(), AudioError> {
        self.running.store(false, Ordering::Relaxed);
        self.stream
            .pause()
            .map_err(|e| AudioError::StreamError(e.to_string()))?;
        info!("Audio playback stopped");
        Ok(())
    }

    /// Send an audio buffer for playback.
    pub fn send(&self, buffer: AudioBuffer) -> Result<(), AudioError> {
        self.sender
            .try_send(buffer)
            .map_err(|_| AudioError::ChannelClosed)
    }

    /// Get the sender for async usage.
    pub fn sender(&self) -> Sender<AudioBuffer> {
        self.sender.clone()
    }

    /// Check if playback is running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Get the device name.
    pub fn device_name(&self) -> &str {
        &self.device_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_creation() {
        // This test may fail on CI without audio devices
        match AudioCapture::new() {
            Ok(capture) => {
                println!("Created capture on: {}", capture.device_name());
            }
            Err(e) => {
                println!("Could not create capture (expected in CI): {}", e);
            }
        }
    }

    #[test]
    fn test_playback_creation() {
        // This test may fail on CI without audio devices
        match AudioPlayback::new() {
            Ok(playback) => {
                println!("Created playback on: {}", playback.device_name());
            }
            Err(e) => {
                println!("Could not create playback (expected in CI): {}", e);
            }
        }
    }
}
