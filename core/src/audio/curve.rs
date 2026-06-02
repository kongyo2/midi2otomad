//! 端点を 0 と 1 に固定したまま、進捗値を指数/対数カーブで曲げる。

const LINEAR_EPSILON: f64 = 1e-6;

/// 正規化された進捗 `x` (`[0, 1]`) をカーブ形状 `tension` で曲げる。
/// `0` は直線、正値は下に膨らむ（ゆっくり始まり加速）、負値は上に膨らむ。
/// 写像 `(e^{k x} - 1) / (e^{k} - 1)` は任意の `k` で単調かつ点対称。
pub fn shape_curve(x: f64, tension: f64) -> f64 {
    let clamped = x.clamp(0.0, 1.0);
    if tension.abs() < LINEAR_EPSILON {
        return clamped;
    }
    (tension * clamped).exp_m1() / tension.exp_m1()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, prec: i32) -> bool {
        (a - b).abs() < 10f64.powi(-prec) / 2.0
    }

    #[test]
    fn identity_at_zero_tension() {
        assert!(close(shape_curve(0.25, 0.0), 0.25, 12));
        assert!(close(shape_curve(0.5, 0.0), 0.5, 12));
        assert!(close(shape_curve(0.8, 0.0), 0.8, 12));
    }

    #[test]
    fn pins_endpoints() {
        assert!(close(shape_curve(0.0, 5.0), 0.0, 12));
        assert!(close(shape_curve(1.0, 5.0), 1.0, 12));
        assert!(close(shape_curve(0.0, -5.0), 0.0, 12));
        assert!(close(shape_curve(1.0, -5.0), 1.0, 12));
    }

    #[test]
    fn bends_with_tension() {
        assert!(shape_curve(0.5, 3.0) < 0.5);
        assert!(shape_curve(0.5, -3.0) > 0.5);
    }

    #[test]
    fn monotonic() {
        let mut prev = f64::NEG_INFINITY;
        for i in 0..=20 {
            let value = shape_curve(i as f64 / 20.0, 4.0);
            assert!(value > prev);
            prev = value;
        }
    }

    #[test]
    fn point_symmetric() {
        for x in [0.1, 0.37, 0.62, 0.95] {
            assert!(close(
                shape_curve(x, 2.5) + shape_curve(1.0 - x, -2.5),
                1.0,
                12
            ));
        }
    }

    #[test]
    fn clamps_progress() {
        assert_eq!(shape_curve(-0.5, 3.0), 0.0);
        assert_eq!(shape_curve(1.5, 3.0), 1.0);
    }
}
