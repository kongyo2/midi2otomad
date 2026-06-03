const F_MIN: f64 = 40.0;
const F_MAX: f64 = 2000.0;
const YIN_THRESHOLD: f64 = 0.15;

pub fn hz_to_midi(hz: f64) -> f64 {
    69.0 + 12.0 * (hz / 440.0).log2()
}

pub fn split_midi(midi: f64) -> (i32, f64) {
    let rounded = midi.round();
    ((rounded as i32).clamp(0, 127), (midi - rounded) * 100.0)
}

fn onset_index(samples: &[f32], peak: f32) -> usize {
    let threshold = peak * 0.2;
    samples
        .iter()
        .position(|&v| v.abs() >= threshold)
        .unwrap_or(0)
        .min(samples.len().saturating_sub(1))
}

fn parabolic_min(d: &[f64], tau: usize) -> f64 {
    if tau == 0 || tau + 1 >= d.len() {
        return tau as f64;
    }
    let (x0, x1, x2) = (d[tau - 1], d[tau], d[tau + 1]);
    let denom = x0 + x2 - 2.0 * x1;
    if denom.abs() < 1e-12 {
        tau as f64
    } else {
        tau as f64 + (x0 - x2) / (2.0 * denom)
    }
}

fn yin_estimate(window: &[f32], sample_rate: f64, tau_min: usize, tau_max: usize) -> Option<f64> {
    let n = window.len();
    let mut diff = vec![0.0f64; tau_max + 1];
    for (tau, slot) in diff.iter_mut().enumerate().skip(1) {
        let mut sum = 0.0;
        for j in 0..(n - tau) {
            let d = window[j] as f64 - window[j + tau] as f64;
            sum += d * d;
        }
        *slot = sum;
    }

    let mut cmnd = vec![1.0f64; tau_max + 1];
    let mut running = 0.0;
    for tau in 1..=tau_max {
        running += diff[tau];
        cmnd[tau] = if running > 0.0 {
            diff[tau] * tau as f64 / running
        } else {
            1.0
        };
    }

    let mut best_tau = 0usize;
    let mut tau = tau_min;
    while tau <= tau_max {
        if cmnd[tau] < YIN_THRESHOLD {
            while tau < tau_max && cmnd[tau + 1] < cmnd[tau] {
                tau += 1;
            }
            best_tau = tau;
            break;
        }
        tau += 1;
    }
    if best_tau == 0 {
        best_tau = (tau_min..=tau_max).min_by(|&a, &b| cmnd[a].total_cmp(&cmnd[b]))?;
    }

    let refined = parabolic_min(&cmnd, best_tau);
    if refined <= 0.0 {
        return None;
    }
    Some(sample_rate / refined)
}

pub fn detect_fundamental_hz(samples: &[f32], sample_rate: f64) -> Option<f64> {
    if sample_rate <= 0.0 {
        return None;
    }
    let peak = samples.iter().fold(0.0f32, |m, &v| m.max(v.abs()));
    if peak < 1e-4 {
        return None;
    }
    let start = onset_index(samples, peak);
    let tail = &samples[start..];
    let n = tail.len();
    if n < 64 {
        return None;
    }
    let tau_min = ((sample_rate / F_MAX).floor() as usize).max(2);
    let tau_max = (n / 2 - 1).min((sample_rate / F_MIN).ceil() as usize);
    if tau_max <= tau_min {
        return None;
    }
    let analysis_len = (2 * tau_max).min(n);
    yin_estimate(&tail[..analysis_len], sample_rate, tau_min, tau_max)
}

pub fn detect_midi_pitch(samples: &[f32], sample_rate: f64) -> Option<f64> {
    detect_fundamental_hz(samples, sample_rate).map(hz_to_midi)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn sine(freq: f64, rate: f64, frames: usize) -> Vec<f32> {
        (0..frames)
            .map(|i| (2.0 * PI * freq * i as f64 / rate).sin() as f32)
            .collect()
    }

    fn harmonic(freq: f64, rate: f64, frames: usize) -> Vec<f32> {
        (0..frames)
            .map(|i| {
                let t = i as f64 / rate;
                let s = (2.0 * PI * freq * t).sin()
                    + 0.5 * (2.0 * PI * 2.0 * freq * t).sin()
                    + 0.33 * (2.0 * PI * 3.0 * freq * t).sin();
                (s / 1.83) as f32
            })
            .collect()
    }

    fn assert_close_midi(detected: f64, expected: f64) {
        assert!(
            (detected - expected).abs() < 0.35,
            "detected MIDI {detected} not close to {expected}"
        );
    }

    #[test]
    fn detects_a440_as_midi_69() {
        let midi = detect_midi_pitch(&sine(440.0, 48000.0, 8192), 48000.0).unwrap();
        assert_close_midi(midi, 69.0);
    }

    #[test]
    fn detects_middle_c() {
        let midi = detect_midi_pitch(&sine(261.6256, 48000.0, 8192), 48000.0).unwrap();
        assert_close_midi(midi, 60.0);
    }

    #[test]
    fn detects_high_and_low_octaves() {
        let high = detect_midi_pitch(&sine(880.0, 48000.0, 8192), 48000.0).unwrap();
        assert_close_midi(high, 81.0);
        let low = detect_midi_pitch(&sine(110.0, 44100.0, 16384), 44100.0).unwrap();
        assert_close_midi(low, 45.0);
    }

    #[test]
    fn finds_fundamental_not_a_harmonic() {
        let midi = detect_midi_pitch(&harmonic(146.83, 48000.0, 16384), 48000.0).unwrap();
        assert_close_midi(midi, 50.0);
    }

    #[test]
    fn tolerates_leading_silence() {
        let mut buf = vec![0.0f32; 4000];
        buf.extend(sine(329.63, 48000.0, 8192));
        let midi = detect_midi_pitch(&buf, 48000.0).unwrap();
        assert_close_midi(midi, 64.0);
    }

    #[test]
    fn rejects_silence_and_short_input() {
        assert!(detect_fundamental_hz(&[0.0f32; 4096], 48000.0).is_none());
        assert!(detect_fundamental_hz(&sine(440.0, 48000.0, 16), 48000.0).is_none());
        assert!(detect_fundamental_hz(&[], 48000.0).is_none());
        assert!(detect_fundamental_hz(&sine(440.0, 48000.0, 4096), 0.0).is_none());
    }

    #[test]
    fn hz_to_midi_and_split() {
        assert!((hz_to_midi(440.0) - 69.0).abs() < 1e-9);
        assert!((hz_to_midi(880.0) - 81.0).abs() < 1e-9);
        let (pitch, cents) = split_midi(60.25);
        assert_eq!(pitch, 60);
        assert!((cents - 25.0).abs() < 1e-9);
        let (pitch, cents) = split_midi(68.6);
        assert_eq!(pitch, 69);
        assert!((cents + 40.0).abs() < 1e-9);
        assert_eq!(split_midi(-5.0).0, 0);
        assert_eq!(split_midi(200.0).0, 127);
    }
}
