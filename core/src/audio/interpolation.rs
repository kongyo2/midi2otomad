pub fn cubic_hermite(y0: f64, y1: f64, y2: f64, y3: f64, t: f64) -> f64 {
    let m1 = (y2 - y0) * 0.5;
    let m2 = (y3 - y1) * 0.5;
    let t2 = t * t;
    let t3 = t2 * t;
    let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
    let h10 = t3 - 2.0 * t2 + t;
    let h01 = -2.0 * t3 + 3.0 * t2;
    let h11 = t3 - t2;
    h00 * y1 + h10 * m1 + h01 * y2 + h11 * m2
}

pub const SINC_HALF: usize = 8;
const SINC_TAPS: usize = 2 * SINC_HALF;

fn blackman(n: f64, taps: f64) -> f64 {
    let w = 2.0 * std::f64::consts::PI * n / (taps - 1.0);
    0.42 - 0.5 * w.cos() + 0.08 * (2.0 * w).cos()
}

fn sinc_window() -> &'static [f64; SINC_TAPS] {
    static WINDOW: std::sync::OnceLock<[f64; SINC_TAPS]> = std::sync::OnceLock::new();
    WINDOW.get_or_init(|| {
        let mut w = [0.0f64; SINC_TAPS];
        for (t, slot) in w.iter_mut().enumerate() {
            *slot = blackman(t as f64, SINC_TAPS as f64);
        }
        w
    })
}

pub fn windowed_sinc(i0: i64, frac: f64, mut at: impl FnMut(i64) -> f64) -> f64 {
    let window = sinc_window();
    let sin_pi_frac = (std::f64::consts::PI * frac).sin();
    let mut acc = 0.0;
    let mut norm = 0.0;
    for (t, &win) in window.iter().enumerate() {
        let offset = t as i64 - SINC_HALF as i64 + 1;
        let x = frac - offset as f64;
        let h = if x.abs() < 1e-9 {
            win
        } else {
            let sign = if offset & 1 == 0 { 1.0 } else { -1.0 };
            sign * sin_pi_frac / (std::f64::consts::PI * x) * win
        };
        acc += at(i0 + offset) * h;
        norm += h;
    }
    if norm.abs() > 1e-12 {
        acc / norm
    } else {
        acc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, prec: i32) -> bool {
        (a - b).abs() < 10f64.powi(-prec) / 2.0
    }

    #[test]
    fn endpoints() {
        assert_eq!(cubic_hermite(5.0, 10.0, 20.0, 40.0, 0.0), 10.0);
        assert_eq!(cubic_hermite(5.0, 10.0, 20.0, 40.0, 1.0), 20.0);
    }

    #[test]
    fn constant() {
        assert!(close(cubic_hermite(0.3, 0.3, 0.3, 0.3, 0.42), 0.3, 12));
    }

    #[test]
    fn reproduces_line() {
        assert!(close(cubic_hermite(1.0, 3.0, 5.0, 7.0, 0.5), 4.0, 12));
    }

    #[test]
    fn reproduces_parabola() {
        assert!(close(cubic_hermite(0.0, 1.0, 4.0, 9.0, 0.5), 2.25, 12));
    }

    #[test]
    fn symmetric_under_reversal() {
        let forward = cubic_hermite(2.0, 7.0, 1.0, 9.0, 0.3);
        let reversed = cubic_hermite(9.0, 1.0, 7.0, 2.0, 0.7);
        assert!(close(forward, reversed, 12));
    }

    #[test]
    fn rings_near_edge() {
        assert!(cubic_hermite(0.0, 0.0, 0.0, 1.0, 0.5) < 0.0);
    }

    #[test]
    fn reproduces_line_across_many_t() {
        for i in 0..=10 {
            let t = i as f64 / 10.0;
            assert!(close(
                cubic_hermite(1.0, 3.0, 5.0, 7.0, t),
                3.0 + 2.0 * t,
                12
            ));
        }
    }

    #[test]
    fn symmetric_step_is_midpoint() {
        assert!(close(cubic_hermite(0.0, 0.0, 1.0, 1.0, 0.5), 0.5, 12));
    }

    #[test]
    fn tangent_matches_central_difference_at_t0() {
        let (y0, y1, y2, y3) = (1.0, 2.0, 5.0, 4.0);
        let h = 1e-6;
        let slope = (cubic_hermite(y0, y1, y2, y3, h) - y1) / h;
        assert!(close(slope, (y2 - y0) * 0.5, 4));
    }

    #[test]
    fn sinc_returns_exact_sample_at_integer_position() {
        let data: Vec<f64> = (0..40).map(|i| (i as f64 * 0.21).sin()).collect();
        let at = |idx: i64| data[idx.clamp(0, 39) as usize];
        for i0 in 8..30 {
            assert!(close(windowed_sinc(i0, 0.0, at), data[i0 as usize], 9));
        }
    }

    fn naive_windowed_sinc(i0: i64, frac: f64, at: impl Fn(i64) -> f64) -> f64 {
        fn sinc(x: f64) -> f64 {
            if x.abs() < 1e-9 {
                1.0
            } else {
                let px = std::f64::consts::PI * x;
                px.sin() / px
            }
        }
        let taps = (2 * SINC_HALF) as i64;
        let mut acc = 0.0;
        let mut norm = 0.0;
        for t in 0..taps {
            let offset = t - SINC_HALF as i64 + 1;
            let h = sinc(frac - offset as f64) * blackman(t as f64, taps as f64);
            acc += at(i0 + offset) * h;
            norm += h;
        }
        if norm.abs() > 1e-12 {
            acc / norm
        } else {
            acc
        }
    }

    #[test]
    fn fast_sinc_matches_naive_reference() {
        let data: Vec<f64> = (0..256).map(|i| (i as f64 * 0.17).sin() * 0.6).collect();
        let at = |idx: i64| data[idx.clamp(0, 255) as usize];
        for step in 0..1000 {
            let frac = step as f64 / 1000.0;
            for &i0 in &[8i64, 40, 99, 200] {
                let fast = windowed_sinc(i0, frac, at);
                let naive = naive_windowed_sinc(i0, frac, at);
                assert!(
                    (fast - naive).abs() < 1e-9,
                    "frac={frac} i0={i0} fast={fast} naive={naive}"
                );
            }
        }
    }

    #[test]
    fn sinc_preserves_constant_signal() {
        let at = |_idx: i64| 0.7;
        assert!(close(windowed_sinc(20, 0.37, at), 0.7, 9));
        assert!(close(windowed_sinc(20, 0.5, at), 0.7, 9));
    }

    #[test]
    fn sinc_reconstructs_bandlimited_sine_better_than_linear() {
        let freq = 0.05;
        let true_at = |x: f64| (2.0 * std::f64::consts::PI * freq * x).sin();
        let data: Vec<f64> = (0..256).map(|i| true_at(i as f64)).collect();
        let at = |idx: i64| data[idx.clamp(0, 255) as usize];
        let mut sinc_err = 0.0;
        let mut linear_err = 0.0;
        for step in 1..20 {
            let pos = 100.0 + step as f64 / 20.0;
            let i0 = pos.floor() as i64;
            let frac = pos - i0 as f64;
            let sinc_val = windowed_sinc(i0, frac, at);
            let linear_val =
                data[i0 as usize] + (data[(i0 + 1) as usize] - data[i0 as usize]) * frac;
            sinc_err += (sinc_val - true_at(pos)).abs();
            linear_err += (linear_val - true_at(pos)).abs();
        }
        assert!(sinc_val_is_finite(&data, at));
        assert!(sinc_err < linear_err);
    }

    fn sinc_val_is_finite(_data: &[f64], at: impl Fn(i64) -> f64 + Copy) -> bool {
        (0..50).all(|s| windowed_sinc(25, s as f64 / 50.0, at).is_finite())
    }
}
