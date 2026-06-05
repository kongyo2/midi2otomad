#[derive(Debug, Clone, Copy)]
pub struct LimiterParams {
    pub ceiling: f64,
    pub attack_ms: f64,
    pub release_ms: f64,
}

impl Default for LimiterParams {
    fn default() -> Self {
        Self {
            ceiling: 0.8,
            attack_ms: 2.0,
            release_ms: 120.0,
        }
    }
}

pub fn soft_clip(x: f64, threshold: f64) -> f64 {
    let abs = x.abs();
    if abs <= threshold {
        return x;
    }
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let over = (abs - threshold) / (1.0 - threshold);
    sign * (threshold + (1.0 - threshold) * over.tanh())
}

fn approach_coef(ms: f64, sample_rate: f64) -> f64 {
    let samples = (ms / 1000.0 * sample_rate).max(1.0);
    1.0 - (-1.0 / samples).exp()
}

pub fn limit_stereo(left: &mut [f32], right: &mut [f32], sample_rate: f64, params: LimiterParams) {
    let n = left.len().min(right.len());
    if n == 0 {
        return;
    }
    let ceiling = params.ceiling.clamp(1e-4, 1.0);

    let peak_at = |i: usize| (left[i] as f64).abs().max((right[i] as f64).abs());
    if !(0..n).any(|i| peak_at(i) > ceiling) {
        return;
    }

    let mut gain: Vec<f64> = (0..n)
        .map(|i| {
            let peak = peak_at(i);
            if peak > ceiling {
                ceiling / peak
            } else {
                1.0
            }
        })
        .collect();

    let atk = approach_coef(params.attack_ms, sample_rate);
    let rel = approach_coef(params.release_ms, sample_rate);

    for i in (0..n - 1).rev() {
        let next = gain[i + 1];
        let pulled = next + (1.0 - next) * atk;
        if pulled < gain[i] {
            gain[i] = pulled;
        }
    }

    let mut g = gain[0];
    for slot in gain.iter_mut() {
        let target = *slot;
        if target < g {
            g = target;
        } else {
            g += (target - g) * rel;
        }
        *slot = g;
    }

    for i in 0..n {
        let g = gain[i];
        left[i] = soft_clip(left[i] as f64 * g, ceiling) as f32;
        right[i] = soft_clip(right[i] as f64 * g, ceiling) as f32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn max_abs(a: &[f32]) -> f64 {
        a.iter().fold(0.0, |p, &v| p.max((v as f64).abs()))
    }

    #[test]
    fn soft_clip_passes_small_signals() {
        assert_eq!(soft_clip(0.5, 0.8), 0.5);
        assert_eq!(soft_clip(-0.5, 0.8), -0.5);
        assert!(soft_clip(2.0, 0.8) < 1.0);
        assert!(soft_clip(2.0, 0.8) > 0.8);
        assert!(soft_clip(-2.0, 0.8) > -1.0);
    }

    #[test]
    fn transparent_below_ceiling() {
        let mut l = vec![0.3f32, -0.4, 0.5, -0.2, 0.1];
        let mut r = vec![-0.1f32, 0.2, -0.3, 0.4, -0.5];
        let lc = l.clone();
        let rc = r.clone();
        limit_stereo(&mut l, &mut r, 48000.0, LimiterParams::default());
        assert_eq!(l, lc);
        assert_eq!(r, rc);
    }

    #[test]
    fn holds_ceiling_on_constant_signal() {
        let mut l = vec![1.0f32; 4000];
        let mut r = vec![1.0f32; 4000];
        limit_stereo(
            &mut l,
            &mut r,
            48000.0,
            LimiterParams {
                ceiling: 0.5,
                ..Default::default()
            },
        );
        assert!(max_abs(&l) <= 0.5 + 1e-6);
        assert!((l[2000] as f64 - 0.5).abs() < 1e-3);
    }

    #[test]
    fn lower_ceiling_attenuates_more() {
        let make = || vec![1.0f32; 2000];
        let limit_to = |c: f64| {
            let (mut l, mut r) = (make(), make());
            limit_stereo(
                &mut l,
                &mut r,
                48000.0,
                LimiterParams {
                    ceiling: c,
                    ..Default::default()
                },
            );
            max_abs(&l)
        };
        assert!(limit_to(0.5) < limit_to(0.95));
    }

    #[test]
    fn never_exceeds_ceiling_through_transient() {
        let mut l = vec![0.1f32; 2000];
        let mut r = vec![0.1f32; 2000];
        for v in l.iter_mut().skip(900).take(50) {
            *v = 4.0;
        }
        for v in r.iter_mut().skip(900).take(50) {
            *v = 4.0;
        }
        limit_stereo(
            &mut l,
            &mut r,
            48000.0,
            LimiterParams {
                ceiling: 0.8,
                ..Default::default()
            },
        );
        assert!(max_abs(&l) <= 0.8 + 1e-6, "peak leaked: {}", max_abs(&l));
        assert!(l.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn gain_ducks_before_the_peak() {
        let mut l = vec![0.2f32; 4000];
        let mut r = vec![0.2f32; 4000];
        for v in l.iter_mut().skip(2000).take(10) {
            *v = 5.0;
        }
        for v in r.iter_mut().skip(2000).take(10) {
            *v = 5.0;
        }
        limit_stereo(
            &mut l,
            &mut r,
            48000.0,
            LimiterParams {
                ceiling: 0.8,
                attack_ms: 3.0,
                release_ms: 120.0,
            },
        );
        assert!((l[1990] as f64).abs() < 0.2);
    }

    #[test]
    fn stereo_linked_preserves_balance() {
        let mut l = vec![2.0f32; 1000];
        let mut r = vec![1.0f32; 1000];
        limit_stereo(&mut l, &mut r, 48000.0, LimiterParams::default());
        let ratio = l[500] as f64 / r[500] as f64;
        assert!((ratio - 2.0).abs() < 1e-3, "ratio drifted: {ratio}");
    }

    #[test]
    fn handles_empty_and_single() {
        limit_stereo(&mut [], &mut [], 48000.0, LimiterParams::default());
        let mut l = vec![3.0f32];
        let mut r = vec![3.0f32];
        limit_stereo(&mut l, &mut r, 48000.0, LimiterParams::default());
        assert!(max_abs(&l) <= 0.8 + 1e-6);
    }
}
