use crate::schema::FilterType;
use std::f64::consts::PI;

#[derive(Debug, Clone, Copy)]
pub struct BiquadCoeffs {
    pub b0: f64,
    pub b1: f64,
    pub b2: f64,
    pub a1: f64,
    pub a2: f64,
}

struct RawCoeffs {
    b0: f64,
    b1: f64,
    b2: f64,
    a0: f64,
    a1: f64,
    a2: f64,
}

struct DesignTerms {
    cosw0: f64,
    alpha: f64,
    a: f64,
    sqrt_a: f64,
}

fn raw_coeffs(kind: FilterType, t: &DesignTerms) -> RawCoeffs {
    let DesignTerms {
        cosw0,
        alpha,
        a,
        sqrt_a,
    } = *t;
    match kind {
        FilterType::Lowpass => RawCoeffs {
            b0: (1.0 - cosw0) / 2.0,
            b1: 1.0 - cosw0,
            b2: (1.0 - cosw0) / 2.0,
            a0: 1.0 + alpha,
            a1: -2.0 * cosw0,
            a2: 1.0 - alpha,
        },
        FilterType::Highpass => RawCoeffs {
            b0: (1.0 + cosw0) / 2.0,
            b1: -(1.0 + cosw0),
            b2: (1.0 + cosw0) / 2.0,
            a0: 1.0 + alpha,
            a1: -2.0 * cosw0,
            a2: 1.0 - alpha,
        },
        FilterType::Bandpass => RawCoeffs {
            b0: alpha,
            b1: 0.0,
            b2: -alpha,
            a0: 1.0 + alpha,
            a1: -2.0 * cosw0,
            a2: 1.0 - alpha,
        },
        FilterType::Notch => RawCoeffs {
            b0: 1.0,
            b1: -2.0 * cosw0,
            b2: 1.0,
            a0: 1.0 + alpha,
            a1: -2.0 * cosw0,
            a2: 1.0 - alpha,
        },
        FilterType::Allpass => RawCoeffs {
            b0: 1.0 - alpha,
            b1: -2.0 * cosw0,
            b2: 1.0 + alpha,
            a0: 1.0 + alpha,
            a1: -2.0 * cosw0,
            a2: 1.0 - alpha,
        },
        FilterType::Peaking => RawCoeffs {
            b0: 1.0 + alpha * a,
            b1: -2.0 * cosw0,
            b2: 1.0 - alpha * a,
            a0: 1.0 + alpha / a,
            a1: -2.0 * cosw0,
            a2: 1.0 - alpha / a,
        },
        FilterType::Lowshelf => {
            let two_sqrt_a_alpha = 2.0 * sqrt_a * alpha;
            RawCoeffs {
                b0: a * (a + 1.0 - (a - 1.0) * cosw0 + two_sqrt_a_alpha),
                b1: 2.0 * a * (a - 1.0 - (a + 1.0) * cosw0),
                b2: a * (a + 1.0 - (a - 1.0) * cosw0 - two_sqrt_a_alpha),
                a0: a + 1.0 + (a - 1.0) * cosw0 + two_sqrt_a_alpha,
                a1: -2.0 * (a - 1.0 + (a + 1.0) * cosw0),
                a2: a + 1.0 + (a - 1.0) * cosw0 - two_sqrt_a_alpha,
            }
        }
        FilterType::Highshelf => {
            let two_sqrt_a_alpha = 2.0 * sqrt_a * alpha;
            RawCoeffs {
                b0: a * (a + 1.0 + (a - 1.0) * cosw0 + two_sqrt_a_alpha),
                b1: -2.0 * a * (a - 1.0 + (a + 1.0) * cosw0),
                b2: a * (a + 1.0 + (a - 1.0) * cosw0 - two_sqrt_a_alpha),
                a0: a + 1.0 - (a - 1.0) * cosw0 + two_sqrt_a_alpha,
                a1: 2.0 * (a - 1.0 - (a + 1.0) * cosw0),
                a2: a + 1.0 - (a - 1.0) * cosw0 - two_sqrt_a_alpha,
            }
        }
    }
}

pub fn design_biquad(
    kind: FilterType,
    freq_hz: f64,
    sample_rate: f64,
    q: f64,
    gain_db: f64,
) -> BiquadCoeffs {
    let w0 = (2.0 * PI * freq_hz) / sample_rate;
    let cosw0 = w0.cos();
    let alpha = w0.sin() / (2.0 * q.max(1e-6));
    let a = 10f64.powf(gain_db / 40.0);
    let raw = raw_coeffs(
        kind,
        &DesignTerms {
            cosw0,
            alpha,
            a,
            sqrt_a: a.sqrt(),
        },
    );
    BiquadCoeffs {
        b0: raw.b0 / raw.a0,
        b1: raw.b1 / raw.a0,
        b2: raw.b2 / raw.a0,
        a1: raw.a1 / raw.a0,
        a2: raw.a2 / raw.a0,
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct BiquadState {
    pub x1: f64,
    pub x2: f64,
    pub y1: f64,
    pub y2: f64,
}

pub fn create_biquad_state() -> BiquadState {
    BiquadState::default()
}

pub fn process_biquad_sample(c: &BiquadCoeffs, s: &mut BiquadState, x: f64) -> f64 {
    let y = c.b0 * x + c.b1 * s.x1 + c.b2 * s.x2 - c.a1 * s.y1 - c.a2 * s.y2;
    s.x2 = s.x1;
    s.x1 = x;
    s.y2 = s.y1;
    s.y1 = y;
    y
}

pub fn process_biquad(c: &BiquadCoeffs, input: &[f32]) -> Vec<f32> {
    let mut state = create_biquad_state();
    input
        .iter()
        .map(|&x| process_biquad_sample(c, &mut state, x as f64) as f32)
        .collect()
}

pub fn magnitude_response(c: &BiquadCoeffs, freq_hz: f64, sample_rate: f64) -> f64 {
    let w = (2.0 * PI * freq_hz) / sample_rate;
    let cos1 = w.cos();
    let sin1 = w.sin();
    let cos2 = (2.0 * w).cos();
    let sin2 = (2.0 * w).sin();
    let num_re = c.b0 + c.b1 * cos1 + c.b2 * cos2;
    let num_im = -(c.b1 * sin1 + c.b2 * sin2);
    let den_re = 1.0 + c.a1 * cos1 + c.a2 * cos2;
    let den_im = -(c.a1 * sin1 + c.a2 * sin2);
    num_re.hypot(num_im) / den_re.hypot(den_im)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FS: f64 = 48000.0;

    fn mag_at(kind: FilterType, freq: f64, q: f64, gain_db: f64) -> f64 {
        let coeffs = design_biquad(kind, 1000.0, FS, q, gain_db);
        magnitude_response(&coeffs, freq, FS)
    }

    fn close(a: f64, b: f64, prec: i32) -> bool {
        (a - b).abs() < 10f64.powi(-prec) / 2.0
    }

    #[test]
    fn lowpass() {
        assert!(close(mag_at(FilterType::Lowpass, 0.0, 0.707, 0.0), 1.0, 6));
        assert!(mag_at(FilterType::Lowpass, 20000.0, 0.707, 0.0) < 0.05);
    }

    #[test]
    fn highpass() {
        assert!(close(mag_at(FilterType::Highpass, 0.0, 0.707, 0.0), 0.0, 6));
        assert!(mag_at(FilterType::Highpass, 20000.0, 0.707, 0.0) > 0.9);
    }

    #[test]
    fn bandpass() {
        let center = mag_at(FilterType::Bandpass, 1000.0, 0.707, 0.0);
        assert!(center > mag_at(FilterType::Bandpass, 100.0, 0.707, 0.0));
        assert!(center > mag_at(FilterType::Bandpass, 10000.0, 0.707, 0.0));
        assert!(close(mag_at(FilterType::Bandpass, 0.0, 0.707, 0.0), 0.0, 6));
    }

    #[test]
    fn notch() {
        assert!(mag_at(FilterType::Notch, 1000.0, 0.707, 0.0) < 0.01);
        assert!(close(mag_at(FilterType::Notch, 0.0, 0.707, 0.0), 1.0, 6));
    }

    #[test]
    fn peaking() {
        assert!(close(
            mag_at(FilterType::Peaking, 1000.0, 1.0, 12.0),
            10f64.powf(12.0 / 20.0),
            2
        ));
        assert!(close(mag_at(FilterType::Peaking, 0.0, 1.0, 12.0), 1.0, 4));
        assert!(mag_at(FilterType::Peaking, 1000.0, 1.0, -12.0) < 1.0);
    }

    #[test]
    fn shelves() {
        assert!(close(
            mag_at(FilterType::Lowshelf, 0.0, 0.707, 12.0),
            10f64.powf(12.0 / 20.0),
            2
        ));
        assert!(close(
            mag_at(FilterType::Lowshelf, 23000.0, 0.707, 12.0),
            1.0,
            1
        ));
        assert!(close(
            mag_at(FilterType::Highshelf, 23000.0, 0.707, 12.0),
            10f64.powf(12.0 / 20.0),
            1
        ));
        assert!(close(
            mag_at(FilterType::Highshelf, 0.0, 0.707, 12.0),
            1.0,
            4
        ));
    }

    #[test]
    fn allpass() {
        assert!(close(
            mag_at(FilterType::Allpass, 100.0, 0.707, 0.0),
            1.0,
            6
        ));
        assert!(close(
            mag_at(FilterType::Allpass, 1000.0, 0.707, 0.0),
            1.0,
            6
        ));
        assert!(close(
            mag_at(FilterType::Allpass, 10000.0, 0.707, 0.0),
            1.0,
            6
        ));
    }

    #[test]
    fn settles_to_dc_gain() {
        let coeffs = design_biquad(FilterType::Lowpass, 1000.0, FS, 0.707, 0.0);
        let input = vec![1.0f32; 2000];
        let output = process_biquad(&coeffs, &input);
        assert!(close(output[output.len() - 1] as f64, 1.0, 4));
    }

    #[test]
    fn attenuates_nyquist() {
        let coeffs = design_biquad(FilterType::Lowpass, 1000.0, FS, 0.707, 0.0);
        let input: Vec<f32> = (0..512)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect();
        let output = process_biquad(&coeffs, &input);
        let mut peak = 0.0f32;
        for &v in &output[256..] {
            assert!(v.is_finite());
            peak = peak.max(v.abs());
        }
        assert!(peak < 0.1);
    }

    #[test]
    fn matches_manual_run() {
        let coeffs = design_biquad(FilterType::Highpass, 2000.0, FS, 1.2, 0.0);
        let input: Vec<f32> = (0..32).map(|i| (i as f64).sin() as f32).collect();
        let mut state = create_biquad_state();
        let expected: Vec<f32> = input
            .iter()
            .map(|&x| process_biquad_sample(&coeffs, &mut state, x as f64) as f32)
            .collect();
        assert_eq!(process_biquad(&coeffs, &input), expected);
    }

    #[test]
    fn butterworth_is_minus_3db_at_cutoff() {
        let mag = mag_at(
            FilterType::Lowpass,
            1000.0,
            std::f64::consts::FRAC_1_SQRT_2,
            0.0,
        );
        assert!(close(mag, std::f64::consts::FRAC_1_SQRT_2, 3));
    }

    #[test]
    fn bandpass_is_unity_at_center() {
        assert!(close(
            mag_at(FilterType::Bandpass, 1000.0, 1.0, 0.0),
            1.0,
            4
        ));
    }

    #[test]
    fn tiny_q_does_not_produce_nan() {
        let c = design_biquad(FilterType::Lowpass, 1000.0, FS, 0.0, 0.0);
        for v in [c.b0, c.b1, c.b2, c.a1, c.a2] {
            assert!(v.is_finite());
        }
    }

    #[test]
    fn process_empty_input_is_empty() {
        let c = design_biquad(FilterType::Lowpass, 1000.0, FS, 0.707, 0.0);
        assert!(process_biquad(&c, &[]).is_empty());
    }

    #[test]
    fn extreme_gains_stay_finite() {
        for gain in [-24.0, -12.0, 12.0, 24.0] {
            for kind in [
                FilterType::Peaking,
                FilterType::Lowshelf,
                FilterType::Highshelf,
            ] {
                let c = design_biquad(kind, 1000.0, FS, 1.0, gain);
                for v in [c.b0, c.b1, c.b2, c.a1, c.a2] {
                    assert!(v.is_finite());
                }
                assert!(magnitude_response(&c, 1000.0, FS).is_finite());
            }
        }
    }

    #[test]
    fn higher_q_sharpens_peak() {
        let narrow = mag_at(FilterType::Peaking, 1000.0, 8.0, 12.0);
        let wide = mag_at(FilterType::Peaking, 1000.0, 1.0, 12.0);
        let target = 10f64.powf(12.0 / 20.0);
        assert!(close(narrow, target, 1));
        assert!(close(wide, target, 1));
        assert!(
            mag_at(FilterType::Peaking, 1600.0, 8.0, 12.0)
                < mag_at(FilterType::Peaking, 1600.0, 1.0, 12.0)
        );
    }
}
