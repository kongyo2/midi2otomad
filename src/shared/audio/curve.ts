const LINEAR_EPSILON = 1e-6;

/**
 * Bends a normalised progress value `x` (in `[0, 1]`) along an exponential or
 * logarithmic curve while keeping both endpoints pinned at 0 and 1.
 *
 * `tension` selects the shape:
 * - `0` is the identity (a straight line).
 * - positive values bend below the diagonal — a slow start that accelerates,
 *   the natural shape for an exponential attack.
 * - negative values bend above the diagonal — a fast start that eases out,
 *   the natural shape for a decaying release.
 *
 * The mapping `(e^{k x} - 1) / (e^{k} - 1)` is monotonic for every `k` and
 * point-symmetric: `shapeCurve(x, k) + shapeCurve(1 - x, -k) === 1`.
 */
export function shapeCurve(x: number, tension: number): number {
  const clamped = x < 0 ? 0 : x > 1 ? 1 : x;
  if (Math.abs(tension) < LINEAR_EPSILON) {
    return clamped;
  }
  return Math.expm1(tension * clamped) / Math.expm1(tension);
}
