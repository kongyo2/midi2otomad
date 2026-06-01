/**
 * Four-point, third-order cubic Hermite interpolation (Catmull-Rom form).
 *
 * Given four equally spaced samples `y0..y3`, returns the interpolated value at
 * fractional position `t` in `[0, 1]` between the inner pair `y1` (t=0) and `y2`
 * (t=1). The tangents at the inner points are estimated with central
 * differences, which reproduces any polynomial up to degree two exactly and is
 * markedly smoother than linear interpolation when resampling pitch-shifted
 * audio.
 */
export function cubicHermite(y0: number, y1: number, y2: number, y3: number, t: number): number {
  const m1 = (y2 - y0) * 0.5;
  const m2 = (y3 - y1) * 0.5;
  const t2 = t * t;
  const t3 = t2 * t;
  const h00 = 2 * t3 - 3 * t2 + 1;
  const h10 = t3 - 2 * t2 + t;
  const h01 = -2 * t3 + 3 * t2;
  const h11 = t3 - t2;
  return h00 * y1 + h10 * m1 + h01 * y2 + h11 * m2;
}
