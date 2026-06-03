use super::filter::{create_biquad_state, design_biquad, process_biquad_sample};
use super::interpolation::cubic_hermite;
use crate::schema::FilterType;
use std::f64::consts::FRAC_1_SQRT_2;

fn clamp_index(index: i64, length: usize) -> usize {
    if index < 0 {
        0
    } else if index as usize >= length {
        length - 1
    } else {
        index as usize
    }
}

fn anti_alias_lowpass(input: &[f32], src_rate: f64, cutoff_hz: f64) -> Vec<f32> {
    let coeffs = design_biquad(FilterType::Lowpass, cutoff_hz, src_rate, FRAC_1_SQRT_2, 0.0);
    let mut stage1 = create_biquad_state();
    let mut stage2 = create_biquad_state();
    input
        .iter()
        .map(|&x| {
            let s1 = process_biquad_sample(&coeffs, &mut stage1, x as f64);
            process_biquad_sample(&coeffs, &mut stage2, s1) as f32
        })
        .collect()
}

pub fn resample_channel(input: &[f32], src_rate: f64, dst_rate: f64) -> Vec<f32> {
    let ratio = src_rate / dst_rate;
    let source: Vec<f32> = if dst_rate < src_rate {
        anti_alias_lowpass(input, src_rate, 0.45 * dst_rate)
    } else {
        input.to_vec()
    };
    let out_length = ((input.len() as f64 / ratio).round() as usize).max(1);
    let len = source.len();
    let mut out = vec![0.0f32; out_length];
    for (i, slot) in out.iter_mut().enumerate() {
        let pos = i as f64 * ratio;
        let base = pos.floor();
        let frac = pos - base;
        let base = base as i64;
        let y0 = source[clamp_index(base - 1, len)] as f64;
        let y1 = source[clamp_index(base, len)] as f64;
        let y2 = source[clamp_index(base + 1, len)] as f64;
        let y3 = source[clamp_index(base + 2, len)] as f64;
        *slot = cubic_hermite(y0, y1, y2, y3, frac) as f32;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(freq_hz: f64, sample_rate: f64, frames: usize) -> Vec<f32> {
        (0..frames)
            .map(|i| ((2.0 * std::f64::consts::PI * freq_hz * i as f64) / sample_rate).sin() as f32)
            .collect()
    }

    fn rms(arr: &[f32], start: usize) -> f64 {
        let sum: f64 = arr[start..].iter().map(|&v| (v as f64) * (v as f64)).sum();
        (sum / (arr.len() - start).max(1) as f64).sqrt()
    }

    #[test]
    fn halves_when_dropping_rate() {
        assert_eq!(
            resample_channel(&vec![0.0f32; 100], 48000.0, 24000.0).len(),
            50
        );
    }

    #[test]
    fn doubles_when_raising_rate() {
        assert_eq!(resample_channel(&[0.0f32; 50], 24000.0, 48000.0).len(), 100);
    }

    #[test]
    fn identity_at_same_rate() {
        let input = tone(1000.0, 48000.0, 64);
        let out = resample_channel(&input, 48000.0, 48000.0);
        assert_eq!(out.len(), 64);
        for i in 0..input.len() {
            assert!((out[i] - input[i]).abs() < 1e-5);
        }
    }

    #[test]
    fn anti_aliases_downsampling() {
        let (src, dst, frames) = (48000.0, 24000.0, 4800);
        let high = resample_channel(&tone(18000.0, src, frames), src, dst);
        let low = resample_channel(&tone(1000.0, src, frames), src, dst);
        assert!(rms(&high, high.len() / 2) < 0.2);
        assert!(rms(&low, low.len() / 2) > 0.45);
    }

    #[test]
    fn keeps_output_finite() {
        let out = resample_channel(&tone(5000.0, 96000.0, 960), 96000.0, 48000.0);
        assert!(out.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn single_sample_input() {
        let out = resample_channel(&[0.5f32], 48000.0, 48000.0);
        assert_eq!(out.len(), 1);
        assert!((out[0] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn preserves_constant_when_upsampling() {
        let out = resample_channel(&[0.5f32; 50], 24000.0, 48000.0);
        assert_eq!(out.len(), 100);
        assert!(out.iter().all(|&v| (v - 0.5).abs() < 1e-5));
    }

    #[test]
    fn output_length_follows_ratio() {
        assert_eq!(
            resample_channel(&[0.0f32; 300], 48000.0, 16000.0).len(),
            100
        );
        assert_eq!(resample_channel(&[0.0f32; 25], 12000.0, 48000.0).len(), 100);
    }

    #[test]
    fn preserves_low_frequency_amplitude() {
        let src = tone(500.0, 48000.0, 4800);
        let out = resample_channel(&src, 48000.0, 24000.0);
        assert!(rms(&out, out.len() / 2) > 0.6);
    }

    #[test]
    fn upsample_then_downsample_roundtrip_is_close() {
        let src = tone(800.0, 24000.0, 2400);
        let up = resample_channel(&src, 24000.0, 48000.0);
        let back = resample_channel(&up, 48000.0, 24000.0);
        assert_eq!(back.len(), src.len());
        assert!((rms(&back, src.len() / 2) - rms(&src, src.len() / 2)).abs() < 0.1);
    }
}
