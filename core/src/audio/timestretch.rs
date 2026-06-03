//! WSOLA（Waveform Similarity Overlap-Add）方式のタイムストレッチ。音程を変えずに
//! 長さだけを `factor` 倍にする。各合成フレームの自然な続きに最も似た解析フレームを
//! 近傍探索で選び、相互相関でアラインしてからオーバーラップ加算するため、定常音の
//! 位相が保たれて周波数がずれない。ワンショットをノート長へ伸ばす用途に使う。

/// Hann 窓（`sin²`）。50% オーバーラップで滑らかにクロスフェードする。
fn hann(n: usize) -> Vec<f32> {
    if n <= 1 {
        return vec![1.0; n.max(1)];
    }
    (0..n)
        .map(|i| {
            let s = (std::f64::consts::PI * i as f64 / (n as f64 - 1.0)).sin();
            (s * s) as f32
        })
        .collect()
}

/// 入力が短すぎて窓を組めないときの素朴な伸縮（最近傍ホールド）。
fn resize_nearest(input: &[f32], factor: f64) -> Vec<f32> {
    let n = input.len();
    let out_len = ((n as f64) * factor).round() as usize;
    if n == 0 || out_len == 0 {
        return vec![0.0; out_len];
    }
    (0..out_len)
        .map(|i| {
            let pos = i as f64 / factor;
            input[(pos.floor() as usize).min(n - 1)]
        })
        .collect()
}

/// 理想解析位置 `base` の近傍 ±`search` で、直前フレームの続き `target` に最も似た
/// 開始位置のずれ（相互相関最大）を返す。
fn best_offset(input: &[f32], base: isize, frame: usize, search: usize, target: &[f32]) -> isize {
    let n = input.len() as isize;
    let frame_i = frame as isize;
    let step = (frame / 128).max(1);
    let mut best = 0isize;
    let mut best_score = f64::NEG_INFINITY;
    let mut delta = -(search as isize);
    while delta <= search as isize {
        let start = base + delta;
        if start >= 0 && start + frame_i <= n {
            let s = start as usize;
            let mut dot = 0.0f64;
            let mut k = 0;
            while k < frame {
                dot += input[s + k] as f64 * target[k] as f64;
                k += step;
            }
            if dot > best_score {
                best_score = dot;
                best = delta;
            }
        }
        delta += 1;
    }
    best
}

/// `input` の長さを `factor` 倍に時間伸縮する（音程は不変）。`factor > 1` で長く。
pub fn time_stretch(input: &[f32], factor: f64, sample_rate: f64) -> Vec<f32> {
    let n = input.len();
    if !factor.is_finite() || factor <= 0.0 {
        return input.to_vec();
    }
    if (factor - 1.0).abs() < 1e-6 {
        return input.to_vec();
    }
    let out_len = ((n as f64) * factor).round() as usize;
    if out_len == 0 {
        return Vec::new();
    }
    if n < 128 {
        return resize_nearest(input, factor);
    }

    let frame = ((sample_rate * 0.040) as usize)
        .clamp(128, 2048)
        .min(n / 2)
        .max(64);
    let syn_hop = (frame / 2).max(1);
    let ana_hop = (syn_hop as f64 / factor).max(1.0);
    let search = (frame / 4).min(256);

    let window = hann(frame);
    let mut out = vec![0.0f32; out_len + frame];
    let mut norm = vec![0.0f32; out_len + frame];

    let mut ana_pos = 0.0f64;
    let mut syn_pos = 0usize;
    let mut target: Option<Vec<f32>> = None;

    while syn_pos < out_len {
        let base = ana_pos.round() as isize;
        let delta = match &target {
            None => 0,
            Some(t) => best_offset(input, base, frame, search, t),
        };
        let start = (base + delta).clamp(0, n as isize - 1) as usize;

        for k in 0..frame {
            let idx = start + k;
            let s = if idx < n { input[idx] } else { 0.0 };
            let w = window[k];
            out[syn_pos + k] += s * w;
            norm[syn_pos + k] += w;
        }

        // 次に一致させたい「自然な続き」は、今置いたフレームから合成ホップ進めた区間。
        let cont = start + syn_hop;
        let mut next_target = vec![0.0f32; frame];
        for (k, slot) in next_target.iter_mut().enumerate() {
            let idx = cont + k;
            *slot = if idx < n { input[idx] } else { 0.0 };
        }
        target = Some(next_target);

        syn_pos += syn_hop;
        ana_pos += ana_hop;
    }

    for i in 0..out_len {
        let g = norm[i];
        if g > 1e-6 {
            out[i] /= g;
        }
    }
    out.truncate(out_len);
    out
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

    fn zero_crossings(x: &[f32]) -> usize {
        x.windows(2)
            .filter(|w| (w[0] <= 0.0 && w[1] > 0.0) || (w[0] >= 0.0 && w[1] < 0.0))
            .count()
    }

    #[test]
    fn lengthens_by_factor() {
        let input = sine(440.0, 48000.0, 4800);
        let out = time_stretch(&input, 2.0, 48000.0);
        let expected = (4800.0 * 2.0) as usize;
        assert!((out.len() as i64 - expected as i64).abs() <= 2);
        assert!(out.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn shortens_by_factor() {
        let input = sine(440.0, 48000.0, 4800);
        let out = time_stretch(&input, 0.5, 48000.0);
        assert!((out.len() as i64 - 2400).abs() <= 2);
        assert!(out.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn factor_one_is_identity() {
        let input = sine(330.0, 48000.0, 2000);
        let out = time_stretch(&input, 1.0, 48000.0);
        assert_eq!(out, input);
    }

    #[test]
    fn preserves_pitch_when_stretching() {
        // 周波数が保たれるなら、ゼロ交差「密度」（交差数/長さ）は伸縮前後でほぼ等しい。
        let sr = 48000.0;
        let input = sine(440.0, sr, 4800);
        let out = time_stretch(&input, 1.8, sr);
        let in_rate = zero_crossings(&input) as f64 / input.len() as f64;
        let out_rate = zero_crossings(&out) as f64 / out.len() as f64;
        assert!(
            (in_rate - out_rate).abs() / in_rate < 0.1,
            "in {in_rate} out {out_rate}"
        );
    }

    #[test]
    fn handles_tiny_input() {
        let out = time_stretch(&[0.1, -0.2, 0.3], 3.0, 48000.0);
        assert_eq!(out.len(), 9);
        assert!(out.iter().all(|v| v.is_finite()));

        assert!(time_stretch(&[], 2.0, 48000.0).is_empty());
        assert_eq!(time_stretch(&[0.5; 100], 0.0, 48000.0), vec![0.5; 100]);
    }
}
