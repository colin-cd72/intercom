//! Thread-safe audio handle.
//!
//! This module provides a Send + Sync wrapper around cpal streams by keeping
//! them in a dedicated thread and communicating via channels.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use crossbeam_channel::{bounded, Receiver, Sender};
use tracing::{debug, error, info};

use crate::cpal_backend::{AudioCapture, AudioPlayback};
use crate::{AudioBuffer, AudioError};

/// Commands sent to the audio thread.
enum AudioCommand {
    StartCapture,
    StopCapture,
    StartPlayback,
    StopPlayback,
    SetInputDevice(String),
    SetOutputDevice(String),
    SendPlayback(AudioBuffer),
    Shutdown,
}

/// Encoded audio frame from capture.
#[derive(Debug, Clone)]
pub struct EncodedFrame {
    pub data: Vec<u8>,
    pub timestamp: u64,
}

/// Decoded audio frame for playback.
#[derive(Debug, Clone)]
pub struct DecodedFrame {
    pub samples: Vec<f32>,
}

/// Audio handle that provides Send + Sync access to audio streams.
///
/// The actual cpal streams are owned by a dedicated audio thread, making
/// this handle safe to use across threads.
pub struct AudioHandle {
    command_tx: Sender<AudioCommand>,
    capture_rx: Receiver<AudioBuffer>,
    running: Arc<AtomicBool>,
    thread_handle: Option<JoinHandle<()>>,
}

impl AudioHandle {
    /// Create a new audio handle with optional device names.
    pub fn new(
        input_device: Option<&str>,
        output_device: Option<&str>,
    ) -> Result<Self, AudioError> {
        let (command_tx, command_rx) = bounded::<AudioCommand>(64);
        let (capture_tx, capture_rx) = bounded::<AudioBuffer>(32);
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        let input_name = input_device.map(String::from);
        let output_name = output_device.map(String::from);

        let thread_handle = thread::Builder::new()
            .name("intercom-audio".to_string())
            .spawn(move || {
                audio_thread(command_rx, capture_tx, input_name, output_name, running_clone);
            })
            .map_err(|e| AudioError::StreamError(format!("Failed to spawn audio thread: {}", e)))?;

        Ok(Self {
            command_tx,
            capture_rx,
            running,
            thread_handle: Some(thread_handle),
        })
    }

    /// Start audio capture.
    pub fn start_capture(&self) -> Result<(), AudioError> {
        self.command_tx
            .send(AudioCommand::StartCapture)
            .map_err(|_| AudioError::ChannelClosed)
    }

    /// Stop audio capture.
    pub fn stop_capture(&self) -> Result<(), AudioError> {
        self.command_tx
            .send(AudioCommand::StopCapture)
            .map_err(|_| AudioError::ChannelClosed)
    }

    /// Start audio playback.
    pub fn start_playback(&self) -> Result<(), AudioError> {
        self.command_tx
            .send(AudioCommand::StartPlayback)
            .map_err(|_| AudioError::ChannelClosed)
    }

    /// Stop audio playback.
    pub fn stop_playback(&self) -> Result<(), AudioError> {
        self.command_tx
            .send(AudioCommand::StopPlayback)
            .map_err(|_| AudioError::ChannelClosed)
    }

    /// Set the input device.
    pub fn set_input_device(&self, device_name: &str) -> Result<(), AudioError> {
        self.command_tx
            .send(AudioCommand::SetInputDevice(device_name.to_string()))
            .map_err(|_| AudioError::ChannelClosed)
    }

    /// Set the output device.
    pub fn set_output_device(&self, device_name: &str) -> Result<(), AudioError> {
        self.command_tx
            .send(AudioCommand::SetOutputDevice(device_name.to_string()))
            .map_err(|_| AudioError::ChannelClosed)
    }

    /// Send audio buffer for playback.
    pub fn send_playback(&self, buffer: AudioBuffer) -> Result<(), AudioError> {
        self.command_tx
            .send(AudioCommand::SendPlayback(buffer))
            .map_err(|_| AudioError::ChannelClosed)
    }

    /// Receive captured audio buffer (blocking).
    pub fn recv_capture(&self) -> Result<AudioBuffer, AudioError> {
        self.capture_rx.recv().map_err(|_| AudioError::ChannelClosed)
    }

    /// Try to receive captured audio buffer (non-blocking).
    pub fn try_recv_capture(&self) -> Option<AudioBuffer> {
        self.capture_rx.try_recv().ok()
    }

    /// Get the capture receiver for direct access.
    pub fn capture_receiver(&self) -> &Receiver<AudioBuffer> {
        &self.capture_rx
    }

    /// Check if the audio thread is running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

impl Drop for AudioHandle {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        let _ = self.command_tx.send(AudioCommand::Shutdown);

        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

/// The audio thread function.
fn audio_thread(
    command_rx: Receiver<AudioCommand>,
    capture_tx: Sender<AudioBuffer>,
    input_device: Option<String>,
    output_device: Option<String>,
    running: Arc<AtomicBool>,
) {
    let mut capture: Option<AudioCapture> = None;
    let mut playback: Option<AudioPlayback> = None;

    // Initialize capture device
    if let Err(e) = init_capture(&mut capture, input_device.as_deref()) {
        error!("Failed to initialize capture: {}", e);
    }

    // Initialize playback device
    if let Err(e) = init_playback(&mut playback, output_device.as_deref()) {
        error!("Failed to initialize playback: {}", e);
    }

    info!("Audio thread started");

    while running.load(Ordering::Relaxed) {
        // Process commands (non-blocking with timeout)
        match command_rx.recv_timeout(std::time::Duration::from_millis(1)) {
            Ok(cmd) => match cmd {
                AudioCommand::StartCapture => {
                    if let Some(ref c) = capture {
                        if let Err(e) = c.start() {
                            error!("Failed to start capture: {}", e);
                        } else {
                            debug!("Capture started");
                        }
                    }
                }
                AudioCommand::StopCapture => {
                    if let Some(ref c) = capture {
                        if let Err(e) = c.stop() {
                            error!("Failed to stop capture: {}", e);
                        } else {
                            debug!("Capture stopped");
                        }
                    }
                }
                AudioCommand::StartPlayback => {
                    if let Some(ref p) = playback {
                        if let Err(e) = p.start() {
                            error!("Failed to start playback: {}", e);
                        } else {
                            debug!("Playback started");
                        }
                    }
                }
                AudioCommand::StopPlayback => {
                    if let Some(ref p) = playback {
                        if let Err(e) = p.stop() {
                            error!("Failed to stop playback: {}", e);
                        } else {
                            debug!("Playback stopped");
                        }
                    }
                }
                AudioCommand::SetInputDevice(name) => {
                    if let Err(e) = init_capture(&mut capture, Some(&name)) {
                        error!("Failed to set input device {}: {}", name, e);
                    }
                }
                AudioCommand::SetOutputDevice(name) => {
                    if let Err(e) = init_playback(&mut playback, Some(&name)) {
                        error!("Failed to set output device {}: {}", name, e);
                    }
                }
                AudioCommand::SendPlayback(buffer) => {
                    if let Some(ref p) = playback {
                        if let Err(e) = p.send(buffer) {
                            error!("Failed to send playback buffer: {}", e);
                        }
                    }
                }
                AudioCommand::Shutdown => {
                    debug!("Audio thread shutdown requested");
                    break;
                }
            },
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                // No command, continue
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                debug!("Command channel disconnected");
                break;
            }
        }

        // Forward captured audio
        if let Some(ref c) = capture {
            while let Some(buffer) = c.try_recv() {
                if capture_tx.try_send(buffer).is_err() {
                    // Channel full or closed, drop the buffer
                }
            }
        }
    }

    // Cleanup
    if let Some(ref c) = capture {
        let _ = c.stop();
    }
    if let Some(ref p) = playback {
        let _ = p.stop();
    }

    info!("Audio thread stopped");
}

fn init_capture(capture: &mut Option<AudioCapture>, device: Option<&str>) -> Result<(), AudioError> {
    // Stop existing capture
    if let Some(ref c) = capture {
        let _ = c.stop();
    }

    // Create new capture
    let new_capture = match device {
        Some(name) => AudioCapture::with_device(name)?,
        None => AudioCapture::new()?,
    };

    *capture = Some(new_capture);
    Ok(())
}

fn init_playback(playback: &mut Option<AudioPlayback>, device: Option<&str>) -> Result<(), AudioError> {
    // Stop existing playback
    if let Some(ref p) = playback {
        let _ = p.stop();
    }

    // Create new playback
    let new_playback = match device {
        Some(name) => AudioPlayback::with_device(name)?,
        None => AudioPlayback::new()?,
    };

    *playback = Some(new_playback);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_handle_creation() {
        // This test may fail without audio devices
        let result = AudioHandle::new(None, None);
        match result {
            Ok(handle) => {
                assert!(handle.is_running());
            }
            Err(e) => {
                println!("Could not create audio handle (expected in CI): {}", e);
            }
        }
    }
}
