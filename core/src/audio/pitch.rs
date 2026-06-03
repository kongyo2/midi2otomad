//! 単音素材の基本周波数推定。McLeod Pitch Method（正規化二乗差関数 NSDF）を使い、
//! オクターブ誤りに強い形で基音を求める。ワンショットの「ピッチ自動設定」に使う。

use crate::music::hz_to_midi;

/// 推定結果。検出周波数と、そこから導いた MIDI ノート（基準ピッチ）・セント微調整。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DetectedPitch {
    pub hz: f64,
    pub midi: f64,
    pub base_pitch: i32,
    pub tune_cents: f64,
}

/// もっともエネルギーの高い長さ `win` の窓の開始位置を粗く探す。
fn pick_window(samples: &[f32], win: usize) -> usize {
    let n = samples.len();
    if n <= win {
        return 0;
    }
    let step = ((n - win) / 16).max(1);
    let mut best_off = 0;
    let mut best_energy = -1.0;
    let mut off = 0;
    while off + win <= n {
        let energy: f64 = samples[off..off + win]
            .iter()
            .map(|&v| (v as f64) * (v as f64))
            .sum();
        if energy > best_energy {
            best_energy = energy;
            best_off = off;
        }
        off += step;
    }
    best_off
}

/// チャンネル PCM から基本周波数（Hz）を推定する。無音・非周期・短すぎる場合は `None`。
pub fn detect_pitch_hz(samples: &[f32], sample_rate: f64) -> Option<f64> {
    if sample_rate <= 0.0 {
        return None;
    }
    let win = samples.len().min(4096);
    if win < 256 {
        return None;
    }
    let offset = pick_window(samples, win);
    let frame = &samples[offset..offset + win];

    let mean = frame.iter().map(|&v| v as f64).sum::<f64>() / win as f64;
    let x: Vec<f64> = frame.iter().map(|&v| v as f64 - mean).collect();

    let energy: f64 = x.iter().map(|v| v * v).sum();
    if energy < 1e-6 {
        return None;
    }

    let min_lag = ((sample_rate / 1600.0).floor() as usize).max(2);
    let max_lag = ((sample_rate / 50.0).ceil() as usize).min(win - 2);
    if max_lag <= min_lag + 1 {
        return None;
    }

    // NSDF: nsdf(τ) = 2·Σ x[i]x[i+τ] / Σ (x[i]² + x[i+τ]²)
    let mut nsdf = vec![0.0f64; max_lag + 1];
    for (tau, slot) in nsdf.iter_mut().enumerate().take(max_lag + 1).skip(min_lag) {
        let mut acf = 0.0;
        let mut norm = 0.0;
        for i in 0..(win - tau) {
            acf += x[i] * x[i + tau];
            norm += x[i] * x[i] + x[i + tau] * x[i + tau];
        }
        *slot = if norm > 0.0 { 2.0 * acf / norm } else { 0.0 };
    }

    // 極大（キー・マキシマ）を集め、最大値の 0.8 倍以上で最初に現れるものを基音とする。
    let mut maxima: Vec<usize> = Vec::new();
    let mut global_max = 0.0f64;
    for tau in (min_lag + 1)..max_lag {
        if nsdf[tau] > nsdf[tau - 1] && nsdf[tau] >= nsdf[tau + 1] && nsdf[tau] > 0.0 {
            maxima.push(tau);
            if nsdf[tau] > global_max {
                global_max = nsdf[tau];
            }
        }
    }
    if maxima.is_empty() || global_max < 0.4 {
        return None;
    }
    let threshold = global_max * 0.8;
    let peak = *maxima.iter().find(|&&tau| nsdf[tau] >= threshold)?;

    // 放物線補間でサブサンプル精度のラグを求める。
    let a = nsdf[peak - 1];
    let b = nsdf[peak];
    let c = nsdf[peak + 1];
    let denom = a - 2.0 * b + c;
    let shift = if denom != 0.0 {
        0.5 * (a - c) / denom
    } else {
        0.0
    };
    let tau_est = peak as f64 + shift;
    if tau_est <= 0.0 {
        return None;
    }
    Some(sample_rate / tau_est)
}

/// 基本周波数を推定し、最も近い MIDI ノートとセント微調整に落とし込む。
pub fn detect_base_pitch(samples: &[f32], sample_rate: f64) -> Option<DetectedPitch> {
    let hz = detect_pitch_hz(samples, sample_rate)?;
    let midi = hz_to_midi(hz);
    if !midi.is_finite() {
        return None;
    }
    let nearest = midi.round();
    let tune_cents = ((midi - nearest) * 100.0).clamp(-100.0, 100.0);
    let base_pitch = nearest.clamp(0.0, 127.0) as i32;
    Some(DetectedPitch {
        hz,
        midi,
        base_pitch,
        tune_cents,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn sine(freq: f64, sample_rate: f64, frames: usize) -> Vec<f32> {
        (0..frames)
            .map(|i| (2.0 * PI * freq * i as f64 / sample_rate).sin() as f32)
            .collect()
    }

    fn saw(freq: f64, sample_rate: f64, frames: usize) -> Vec<f32> {
        (0..frames)
            .map(|i| {
                let phase = (freq * i as f64 / sample_rate).fract();
                (2.0 * phase - 1.0) as f32 * 0.7
            })
            .collect()
    }

    fn close_cents(detected: f64, expected: f64) -> bool {
        let cents = 1200.0 * (detected / expected).log2();
        cents.abs() < 35.0
    }

    #[test]
    fn detects_a4_sine() {
        let hz = detect_pitch_hz(&sine(440.0, 48000.0, 8192), 48000.0).unwrap();
        assert!(close_cents(hz, 440.0), "got {hz}");
    }

    #[test]
    fn detects_low_and_high_sines() {
        let low = detect_pitch_hz(&sine(110.0, 48000.0, 8192), 48000.0).unwrap();
        assert!(close_cents(low, 110.0), "got {low}");
        let high = detect_pitch_hz(&sine(880.0, 44100.0, 8192), 44100.0).unwrap();
        assert!(close_cents(high, 880.0), "got {high}");
    }

    #[test]
    fn detects_fundamental_of_rich_sawtooth() {
        // 倍音の豊富なノコギリ波でも基音（オクターブ上に誤らない）を返す。
        let hz = detect_pitch_hz(&saw(220.0, 48000.0, 8192), 48000.0).unwrap();
        assert!(close_cents(hz, 220.0), "got {hz}");
    }

    #[test]
    fn maps_to_midi_note_and_cents() {
        // 440Hz は A4 = MIDI 69、微調整ほぼ 0。
        let d = detect_base_pitch(&sine(440.0, 48000.0, 8192), 48000.0).unwrap();
        assert_eq!(d.base_pitch, 69);
        assert!(d.tune_cents.abs() < 35.0);

        // 少しシャープな 448Hz は A4（+31 cent 付近）。
        let sharp = detect_base_pitch(&sine(448.0, 48000.0, 8192), 48000.0).unwrap();
        assert_eq!(sharp.base_pitch, 69);
        assert!(sharp.tune_cents > 15.0);
    }

    #[test]
    fn rejects_silence_and_noise() {
        assert!(detect_pitch_hz(&vec![0.0f32; 8192], 48000.0).is_none());
        assert!(detect_pitch_hz(&[], 48000.0).is_none());
        assert!(detect_pitch_hz(&sine(440.0, 48000.0, 64), 48000.0).is_none());
        assert!(detect_pitch_hz(&sine(440.0, 0.0, 8192), 0.0).is_none());
    }
}
