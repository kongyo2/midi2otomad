use crate::schema::LfoShape;
use std::f64::consts::PI;

fn wrap01(x: f64) -> f64 {
    x - x.floor()
}

pub fn lfo_value(shape: LfoShape, phase: f64) -> f64 {
    let frac = wrap01(phase);
    match shape {
        LfoShape::Sine => (2.0 * PI * frac).sin(),
        LfoShape::Triangle => 1.0 - 4.0 * (wrap01(frac + 0.25) - 0.5).abs(),
        LfoShape::Square => {
            if frac < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        LfoShape::Saw => 2.0 * frac - 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, prec: i32) -> bool {
        (a - b).abs() < 10f64.powi(-prec) / 2.0
    }

    #[test]
    fn sine_landmarks() {
        assert!(close(lfo_value(LfoShape::Sine, 0.0), 0.0, 9));
        assert!(close(lfo_value(LfoShape::Sine, 0.25), 1.0, 9));
        assert!(close(lfo_value(LfoShape::Sine, 0.5), 0.0, 9));
        assert!(close(lfo_value(LfoShape::Sine, 0.75), -1.0, 9));
    }

    #[test]
    fn triangle_landmarks() {
        assert!(close(lfo_value(LfoShape::Triangle, 0.0), 0.0, 9));
        assert!(close(lfo_value(LfoShape::Triangle, 0.25), 1.0, 9));
        assert!(close(lfo_value(LfoShape::Triangle, 0.5), 0.0, 9));
        assert!(close(lfo_value(LfoShape::Triangle, 0.75), -1.0, 9));
        assert!(close(lfo_value(LfoShape::Triangle, 0.125), 0.5, 9));
    }

    #[test]
    fn square_halves() {
        assert_eq!(lfo_value(LfoShape::Square, 0.25), 1.0);
        assert_eq!(lfo_value(LfoShape::Square, 0.75), -1.0);
    }

    #[test]
    fn saw_ramp() {
        assert!(close(lfo_value(LfoShape::Saw, 0.0), -1.0, 9));
        assert!(close(lfo_value(LfoShape::Saw, 0.5), 0.0, 9));
        assert!(lfo_value(LfoShape::Saw, 0.999) > 0.99);
    }

    #[test]
    fn phase_wrapping() {
        assert!(close(
            lfo_value(LfoShape::Sine, 1.25),
            lfo_value(LfoShape::Sine, 0.25),
            9
        ));
        assert!(close(
            lfo_value(LfoShape::Saw, -0.5),
            lfo_value(LfoShape::Saw, 0.5),
            9
        ));
    }

    #[test]
    fn stays_in_unit_range() {
        for shape in [
            LfoShape::Sine,
            LfoShape::Triangle,
            LfoShape::Square,
            LfoShape::Saw,
        ] {
            for i in 0..64 {
                let value = lfo_value(shape, i as f64 / 64.0);
                assert!((-1.0..=1.0).contains(&value));
            }
        }
    }

    #[test]
    fn square_boundaries() {
        assert_eq!(lfo_value(LfoShape::Square, 0.0), 1.0);
        assert_eq!(lfo_value(LfoShape::Square, 0.49), 1.0);
        assert_eq!(lfo_value(LfoShape::Square, 0.5), -1.0);
        assert_eq!(lfo_value(LfoShape::Square, 0.99), -1.0);
    }

    #[test]
    fn negative_phase_wraps_for_all_shapes() {
        for shape in [
            LfoShape::Sine,
            LfoShape::Triangle,
            LfoShape::Square,
            LfoShape::Saw,
        ] {
            assert!(close(lfo_value(shape, -0.25), lfo_value(shape, 0.75), 9));
            assert!(close(lfo_value(shape, -1.3), lfo_value(shape, 0.7), 9));
        }
    }

    #[test]
    fn triangle_is_continuous_and_peaks() {
        assert!(close(lfo_value(LfoShape::Triangle, 0.25), 1.0, 9));
        assert!(close(lfo_value(LfoShape::Triangle, 0.75), -1.0, 9));
        let a = lfo_value(LfoShape::Triangle, 0.3);
        let b = lfo_value(LfoShape::Triangle, 0.31);
        assert!((a - b).abs() < 0.05);
    }

    #[test]
    fn sine_completes_full_cycle() {
        assert!(close(lfo_value(LfoShape::Sine, 1.0), 0.0, 9));
        assert!(close(
            lfo_value(LfoShape::Sine, 0.1),
            (2.0 * PI * 0.1).sin(),
            9
        ));
    }
}
