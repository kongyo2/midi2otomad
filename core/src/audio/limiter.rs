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
            attack_ms: 1.5,
            release_ms: 120.0,
        }
    }
}

fn approach_coef(ms: f64, sample_rate: f64) -> f32 {
    let samples = (ms / 1000.0 * sample_rate).max(1.0);
    (1.0 - (-1.0 / samples).exp()) as f32
}

pub fn limit_stereo(left: &mut [f32], right: &mut [f32], sample_rate: f64, params: LimiterParams) {
    let n = left.len().min(right.len());
    if n == 0 {
        return;
    }
    let ceiling = params.ceiling.clamp(1e-4, 1.0) as f32;

    if !(0..n).any(|i| left[i].abs().max(right[i].abs()) > ceiling) {
        return;
    }

    let mut gain = vec![1.0f32; n];
    for (i, g) in gain.iter_mut().enumerate() {
        let peak = left[i].abs().max(right[i].abs());
        if peak > ceiling {
            *g = ceiling / peak;
        }
    }

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
    for i in 0..n {
        let target = gain[i];
        g = if target < g {
            target
        } else {
            g + (target - g) * rel
        };
        left[i] *= g;
        right[i] *= g;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn max_abs(a: &[f32]) -> f64 {
        a.iter().fold(0.0, |p, &v| p.max((v as f64).abs()))
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
    fn loud_sine_is_scaled_not_clipped() {
        let n = 24000usize;
        let (freq, rate) = (200.0f64, 48000.0f64);
        let input: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f64::consts::PI * freq * i as f64 / rate).sin() as f32 * 2.0)
            .collect();
        let mut l = input.clone();
        let mut r = input.clone();
        limit_stereo(&mut l, &mut r, rate, LimiterParams::default());

        let s0 = 12000;
        let (inp, out) = (&input[s0..], &l[s0..]);
        let dot: f64 = out
            .iter()
            .zip(inp)
            .map(|(&o, &x)| o as f64 * x as f64)
            .sum();
        let den: f64 = inp.iter().map(|&x| (x as f64) * (x as f64)).sum();
        let scale = dot / den;
        let resid: f64 = out
            .iter()
            .zip(inp)
            .map(|(&o, &x)| {
                let e = o as f64 - scale * x as f64;
                e * e
            })
            .sum::<f64>()
            .sqrt();
        let sig: f64 = out
            .iter()
            .map(|&o| (o as f64) * (o as f64))
            .sum::<f64>()
            .sqrt();
        assert!(
            resid / sig < 0.1,
            "limited sine deviates from a clean scaling (flat-topping?): {}",
            resid / sig
        );
        let peak = max_abs(&l);
        assert!((0.5..=0.8 + 1e-3).contains(&peak), "peak {peak}");
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
