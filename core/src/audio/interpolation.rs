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
}
