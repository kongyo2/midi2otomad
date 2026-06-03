//! ワンショット波形の位相補正。直流オフセット除去と、再生開始を最初の立ち上がりへ
//! 揃える先頭トリムを提供する。リトリガー時の位相をそろえ、アタックを締める。

/// チャンネルから直流オフセット（平均値）を引く。完全な無バイアスなら何もしない。
pub fn remove_dc(channel: &mut [f32]) {
    if channel.is_empty() {
        return;
    }
    let mean = channel.iter().map(|&v| v as f64).sum::<f64>() / channel.len() as f64;
    if mean == 0.0 {
        return;
    }
    let m = mean as f32;
    for v in channel.iter_mut() {
        *v -= m;
    }
}

/// 立ち上がり（ピークの 2% を最初に超える点）の直前のゼロ交差を返す。頭の無音や
/// ゆるい立ち上がりを詰め、クリックを避ける位置で切る。全チャンネルを跨いで判定する。
pub fn leading_trim(channels: &[Vec<f32>]) -> usize {
    let frames = channels.iter().map(|c| c.len()).max().unwrap_or(0);
    if frames == 0 || channels.is_empty() {
        return 0;
    }
    let mut peak = 0.0f32;
    for c in channels {
        for &v in c {
            peak = peak.max(v.abs());
        }
    }
    if peak <= 0.0 {
        return 0;
    }
    let threshold = peak * 0.02;

    let mut onset = 0usize;
    let mut found = false;
    'scan: for i in 0..frames {
        for c in channels {
            if i < c.len() && c[i].abs() >= threshold {
                onset = i;
                found = true;
                break 'scan;
            }
        }
    }
    if !found || onset == 0 {
        return 0;
    }

    // 立ち上がり直前のゼロ交差まで戻る（チャンネル 0 基準）。
    let ch0 = &channels[0];
    let mut z = onset.min(ch0.len().saturating_sub(1));
    while z > 0 {
        let a = ch0.get(z - 1).copied().unwrap_or(0.0);
        let b = ch0.get(z).copied().unwrap_or(0.0);
        if (a <= 0.0 && b >= 0.0) || (a >= 0.0 && b <= 0.0) {
            break;
        }
        z -= 1;
    }
    z
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn tone(freq: f64, frames: usize) -> Vec<f32> {
        (0..frames)
            .map(|i| (2.0 * PI * freq * i as f64 / 48000.0).sin() as f32 * 0.8)
            .collect()
    }

    fn dc(channel: &[f32]) -> f64 {
        channel.iter().map(|&v| v as f64).sum::<f64>() / channel.len().max(1) as f64
    }

    #[test]
    fn removes_constant_bias() {
        let mut ch: Vec<f32> = tone(200.0, 2000).iter().map(|v| v + 0.3).collect();
        assert!(dc(&ch) > 0.25);
        remove_dc(&mut ch);
        assert!(dc(&ch).abs() < 1e-4);
    }

    #[test]
    fn remove_dc_noop_on_centered_signal() {
        let mut constant = vec![0.0f32; 100];
        remove_dc(&mut constant);
        assert!(constant.iter().all(|&v| v == 0.0));
        remove_dc(&mut []);
    }

    #[test]
    fn trims_leading_silence() {
        let mut samples = vec![0.0f32; 300];
        samples.extend(tone(440.0, 2000));
        let trim = leading_trim(&[samples]);
        // 立ち上がり（index 301 付近）の直前のゼロ交差で切る。頭の無音をほぼ詰める。
        assert!((295..=305).contains(&trim), "trim = {trim}");
    }

    #[test]
    fn no_trim_when_attack_is_immediate() {
        // 先頭サンプルが既に立ち上がっている（コサイン: 開始がピーク）なら切らない。
        let samples: Vec<f32> = (0..2000)
            .map(|i| (2.0 * PI * 440.0 * i as f64 / 48000.0 + PI / 2.0).sin() as f32 * 0.8)
            .collect();
        assert_eq!(leading_trim(&[samples]), 0);
    }

    #[test]
    fn handles_empty_and_silent() {
        assert_eq!(leading_trim(&[]), 0);
        assert_eq!(leading_trim(&[vec![0.0f32; 100]]), 0);
        assert_eq!(leading_trim(&[Vec::<f32>::new()]), 0);
    }

    #[test]
    fn trims_consistently_across_channels() {
        let mut left = vec![0.0f32; 200];
        left.extend(tone(330.0, 1000));
        let mut right = vec![0.0f32; 200];
        right.extend(tone(330.0, 1000));
        let trim = leading_trim(&[left, right]);
        assert!((195..=210).contains(&trim), "trim = {trim}");
    }
}
