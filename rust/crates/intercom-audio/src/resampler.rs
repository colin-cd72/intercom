//! Simple audio resampling.

/// Resample audio using linear interpolation.
/// For production, consider using a higher quality resampler like rubato.
pub fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return samples.to_vec();
    }

    let ratio = from_rate as f64 / to_rate as f64;
    let output_len = ((samples.len() as f64) / ratio).ceil() as usize;
    let mut output = Vec::with_capacity(output_len);

    for i in 0..output_len {
        let src_pos = i as f64 * ratio;
        let src_idx = src_pos.floor() as usize;
        let frac = (src_pos - src_idx as f64) as f32;

        let sample = if src_idx + 1 < samples.len() {
            // Linear interpolation
            samples[src_idx] * (1.0 - frac) + samples[src_idx + 1] * frac
        } else if src_idx < samples.len() {
            samples[src_idx]
        } else {
            0.0
        };

        output.push(sample);
    }

    output
}

/// Resample with higher quality using cubic interpolation.
pub fn resample_cubic(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return samples.to_vec();
    }

    let ratio = from_rate as f64 / to_rate as f64;
    let output_len = ((samples.len() as f64) / ratio).ceil() as usize;
    let mut output = Vec::with_capacity(output_len);

    for i in 0..output_len {
        let src_pos = i as f64 * ratio;
        let src_idx = src_pos.floor() as usize;
        let frac = (src_pos - src_idx as f64) as f32;

        // Get four samples for cubic interpolation
        let s0 = if src_idx > 0 {
            samples[src_idx - 1]
        } else {
            samples.get(src_idx).copied().unwrap_or(0.0)
        };
        let s1 = samples.get(src_idx).copied().unwrap_or(0.0);
        let s2 = samples.get(src_idx + 1).copied().unwrap_or(s1);
        let s3 = samples.get(src_idx + 2).copied().unwrap_or(s2);

        // Catmull-Rom cubic interpolation
        let sample = cubic_interpolate(s0, s1, s2, s3, frac);
        output.push(sample);
    }

    output
}

/// Catmull-Rom cubic interpolation.
fn cubic_interpolate(y0: f32, y1: f32, y2: f32, y3: f32, t: f32) -> f32 {
    let a0 = -0.5 * y0 + 1.5 * y1 - 1.5 * y2 + 0.5 * y3;
    let a1 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
    let a2 = -0.5 * y0 + 0.5 * y2;
    let a3 = y1;

    a0 * t * t * t + a1 * t * t + a2 * t + a3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_resample() {
        let input = vec![1.0, 2.0, 3.0, 4.0];
        let output = resample(&input, 48000, 48000);
        assert_eq!(output, input);
    }

    #[test]
    fn test_downsample_2x() {
        let input: Vec<f32> = (0..100).map(|i| i as f32).collect();
        let output = resample(&input, 48000, 24000);
        assert_eq!(output.len(), 50);
    }

    #[test]
    fn test_upsample_2x() {
        let input: Vec<f32> = (0..50).map(|i| i as f32).collect();
        let output = resample(&input, 24000, 48000);
        assert_eq!(output.len(), 100);
    }

    #[test]
    fn test_cubic_vs_linear() {
        let input: Vec<f32> = (0..100).map(|i| (i as f32 * 0.1).sin()).collect();
        let linear = resample(&input, 48000, 44100);
        let cubic = resample_cubic(&input, 48000, 44100);

        // Both should produce similar lengths
        assert_eq!(linear.len(), cubic.len());
    }
}
