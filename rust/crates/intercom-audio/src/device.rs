//! Audio device enumeration and selection.

use cpal::traits::{DeviceTrait, HostTrait};
use tracing::{debug, info, warn};

use crate::AudioError;

/// Type of audio device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    Input,
    Output,
}

/// Audio device information.
#[derive(Debug, Clone)]
pub struct AudioDevice {
    /// Device name.
    pub name: String,
    /// Device type (input or output).
    pub device_type: DeviceType,
    /// Whether this is the default device.
    pub is_default: bool,
    /// Supported sample rates.
    pub sample_rates: Vec<u32>,
    /// Maximum number of channels.
    pub max_channels: u16,
}

impl AudioDevice {
    /// List all available audio devices.
    pub fn list_all() -> Result<Vec<AudioDevice>, AudioError> {
        let host = cpal::default_host();
        let mut devices = Vec::new();

        // Get default device names for comparison
        let default_input = host
            .default_input_device()
            .and_then(|d| d.name().ok());
        let default_output = host
            .default_output_device()
            .and_then(|d| d.name().ok());

        // List input devices
        if let Ok(input_devices) = host.input_devices() {
            for device in input_devices {
                if let Ok(name) = device.name() {
                    let is_default = default_input.as_ref() == Some(&name);
                    let (sample_rates, max_channels) =
                        Self::get_device_capabilities(&device, DeviceType::Input);

                    devices.push(AudioDevice {
                        name,
                        device_type: DeviceType::Input,
                        is_default,
                        sample_rates,
                        max_channels,
                    });
                }
            }
        }

        // List output devices
        if let Ok(output_devices) = host.output_devices() {
            for device in output_devices {
                if let Ok(name) = device.name() {
                    let is_default = default_output.as_ref() == Some(&name);
                    let (sample_rates, max_channels) =
                        Self::get_device_capabilities(&device, DeviceType::Output);

                    devices.push(AudioDevice {
                        name,
                        device_type: DeviceType::Output,
                        is_default,
                        sample_rates,
                        max_channels,
                    });
                }
            }
        }

        info!("Found {} audio devices", devices.len());
        Ok(devices)
    }

    /// List input devices only.
    pub fn list_inputs() -> Result<Vec<AudioDevice>, AudioError> {
        Ok(Self::list_all()?
            .into_iter()
            .filter(|d| d.device_type == DeviceType::Input)
            .collect())
    }

    /// List output devices only.
    pub fn list_outputs() -> Result<Vec<AudioDevice>, AudioError> {
        Ok(Self::list_all()?
            .into_iter()
            .filter(|d| d.device_type == DeviceType::Output)
            .collect())
    }

    /// Get the default input device.
    pub fn default_input() -> Result<AudioDevice, AudioError> {
        Self::list_inputs()?
            .into_iter()
            .find(|d| d.is_default)
            .ok_or(AudioError::NoDevice)
    }

    /// Get the default output device.
    pub fn default_output() -> Result<AudioDevice, AudioError> {
        Self::list_outputs()?
            .into_iter()
            .find(|d| d.is_default)
            .ok_or(AudioError::NoDevice)
    }

    /// Check if the device supports the required sample rate.
    pub fn supports_sample_rate(&self, rate: u32) -> bool {
        self.sample_rates.contains(&rate)
    }

    fn get_device_capabilities(
        device: &cpal::Device,
        device_type: DeviceType,
    ) -> (Vec<u32>, u16) {
        let mut sample_rates = Vec::new();
        let mut max_channels = 0u16;

        // Helper closure to process config ranges
        let mut process_config = |min_rate: u32, max_rate: u32, channels: u16| {
            max_channels = max_channels.max(channels);
            for &rate in &[8000, 16000, 22050, 44100, 48000, 96000] {
                if rate >= min_rate && rate <= max_rate && !sample_rates.contains(&rate) {
                    sample_rates.push(rate);
                }
            }
        };

        match device_type {
            DeviceType::Input => {
                if let Ok(configs) = device.supported_input_configs() {
                    for config in configs {
                        process_config(
                            config.min_sample_rate().0,
                            config.max_sample_rate().0,
                            config.channels(),
                        );
                    }
                }
            }
            DeviceType::Output => {
                if let Ok(configs) = device.supported_output_configs() {
                    for config in configs {
                        process_config(
                            config.min_sample_rate().0,
                            config.max_sample_rate().0,
                            config.channels(),
                        );
                    }
                }
            }
        }

        sample_rates.sort();
        (sample_rates, max_channels)
    }
}

/// Get a cpal device by name.
pub(crate) fn get_cpal_device(
    name: &str,
    device_type: DeviceType,
) -> Result<cpal::Device, AudioError> {
    let host = cpal::default_host();

    let devices = match device_type {
        DeviceType::Input => host.input_devices(),
        DeviceType::Output => host.output_devices(),
    }
    .map_err(|e| AudioError::DeviceNotFound(e.to_string()))?;

    for device in devices {
        if let Ok(device_name) = device.name() {
            if device_name == name {
                return Ok(device);
            }
        }
    }

    Err(AudioError::DeviceNotFound(name.to_string()))
}

/// Get the default cpal device.
pub(crate) fn get_default_device(device_type: DeviceType) -> Result<cpal::Device, AudioError> {
    let host = cpal::default_host();

    match device_type {
        DeviceType::Input => host.default_input_device(),
        DeviceType::Output => host.default_output_device(),
    }
    .ok_or(AudioError::NoDevice)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_devices() {
        // This test may fail on CI without audio devices
        if let Ok(devices) = AudioDevice::list_all() {
            println!("Found {} devices:", devices.len());
            for device in &devices {
                println!(
                    "  {:?}: {} (default: {})",
                    device.device_type, device.name, device.is_default
                );
            }
        }
    }
}
