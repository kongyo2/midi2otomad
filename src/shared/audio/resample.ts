import { cubicHermite } from "./interpolation";
import { createBiquadState, designBiquad, processBiquadSample } from "./filter";

function clampIndex(index: number, length: number): number {
  if (index < 0) {
    return 0;
  }
  if (index >= length) {
    return length - 1;
  }
  return index;
}

/**
 * Two cascaded Butterworth lowpass stages (24 dB/oct) just below the target
 * Nyquist, so energy that would otherwise fold back as aliasing is removed
 * before the source is decimated.
 */
function antiAliasLowpass(input: Float32Array, srcRate: number, cutoffHz: number): Float32Array {
  const coeffs = designBiquad("lowpass", cutoffHz, srcRate, Math.SQRT1_2, 0);
  const stage1 = createBiquadState();
  const stage2 = createBiquadState();
  const out = new Float32Array(input.length);
  for (let i = 0; i < input.length; i += 1) {
    out[i] = processBiquadSample(coeffs, stage2, processBiquadSample(coeffs, stage1, input[i]!));
  }
  return out;
}

/**
 * Resample one channel from `srcRate` to `dstRate` with cubic-hermite
 * interpolation. Downsampling first runs an anti-alias lowpass; upsampling and
 * unity ratios read the source directly.
 */
export function resampleChannel(input: Float32Array, srcRate: number, dstRate: number): Float32Array {
  const ratio = srcRate / dstRate;
  const source = dstRate < srcRate ? antiAliasLowpass(input, srcRate, 0.45 * dstRate) : input;
  const outLength = Math.max(1, Math.round(input.length / ratio));
  const out = new Float32Array(outLength);
  for (let i = 0; i < outLength; i += 1) {
    const pos = i * ratio;
    const base = Math.floor(pos);
    const frac = pos - base;
    const y0 = source[clampIndex(base - 1, source.length)]!;
    const y1 = source[clampIndex(base, source.length)]!;
    const y2 = source[clampIndex(base + 1, source.length)]!;
    const y3 = source[clampIndex(base + 2, source.length)]!;
    out[i] = cubicHermite(y0, y1, y2, y3, frac);
  }
  return out;
}
