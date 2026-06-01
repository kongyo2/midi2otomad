/**
 * Second-order (biquad) IIR filters using the well-known Audio EQ Cookbook
 * formulas by Robert Bristow-Johnson. Coefficients are returned already
 * normalised by `a0`, ready for a Direct Form I difference equation.
 */
export type BiquadType =
  | "lowpass"
  | "highpass"
  | "bandpass"
  | "notch"
  | "peaking"
  | "lowshelf"
  | "highshelf"
  | "allpass";

export interface BiquadCoeffs {
  b0: number;
  b1: number;
  b2: number;
  a1: number;
  a2: number;
}

interface RawCoeffs {
  b0: number;
  b1: number;
  b2: number;
  a0: number;
  a1: number;
  a2: number;
}

interface DesignTerms {
  cosw0: number;
  alpha: number;
  a: number;
  sqrtA: number;
}

const DESIGNERS: Record<BiquadType, (t: DesignTerms) => RawCoeffs> = {
  lowpass: ({ cosw0, alpha }) => ({
    b0: (1 - cosw0) / 2,
    b1: 1 - cosw0,
    b2: (1 - cosw0) / 2,
    a0: 1 + alpha,
    a1: -2 * cosw0,
    a2: 1 - alpha,
  }),
  highpass: ({ cosw0, alpha }) => ({
    b0: (1 + cosw0) / 2,
    b1: -(1 + cosw0),
    b2: (1 + cosw0) / 2,
    a0: 1 + alpha,
    a1: -2 * cosw0,
    a2: 1 - alpha,
  }),
  bandpass: ({ cosw0, alpha }) => ({
    b0: alpha,
    b1: 0,
    b2: -alpha,
    a0: 1 + alpha,
    a1: -2 * cosw0,
    a2: 1 - alpha,
  }),
  notch: ({ cosw0, alpha }) => ({
    b0: 1,
    b1: -2 * cosw0,
    b2: 1,
    a0: 1 + alpha,
    a1: -2 * cosw0,
    a2: 1 - alpha,
  }),
  allpass: ({ cosw0, alpha }) => ({
    b0: 1 - alpha,
    b1: -2 * cosw0,
    b2: 1 + alpha,
    a0: 1 + alpha,
    a1: -2 * cosw0,
    a2: 1 - alpha,
  }),
  peaking: ({ cosw0, alpha, a }) => ({
    b0: 1 + alpha * a,
    b1: -2 * cosw0,
    b2: 1 - alpha * a,
    a0: 1 + alpha / a,
    a1: -2 * cosw0,
    a2: 1 - alpha / a,
  }),
  lowshelf: ({ cosw0, alpha, a, sqrtA }) => {
    const twoSqrtAAlpha = 2 * sqrtA * alpha;
    return {
      b0: a * (a + 1 - (a - 1) * cosw0 + twoSqrtAAlpha),
      b1: 2 * a * (a - 1 - (a + 1) * cosw0),
      b2: a * (a + 1 - (a - 1) * cosw0 - twoSqrtAAlpha),
      a0: a + 1 + (a - 1) * cosw0 + twoSqrtAAlpha,
      a1: -2 * (a - 1 + (a + 1) * cosw0),
      a2: a + 1 + (a - 1) * cosw0 - twoSqrtAAlpha,
    };
  },
  highshelf: ({ cosw0, alpha, a, sqrtA }) => {
    const twoSqrtAAlpha = 2 * sqrtA * alpha;
    return {
      b0: a * (a + 1 + (a - 1) * cosw0 + twoSqrtAAlpha),
      b1: -2 * a * (a - 1 + (a + 1) * cosw0),
      b2: a * (a + 1 + (a - 1) * cosw0 - twoSqrtAAlpha),
      a0: a + 1 - (a - 1) * cosw0 + twoSqrtAAlpha,
      a1: 2 * (a - 1 - (a + 1) * cosw0),
      a2: a + 1 - (a - 1) * cosw0 - twoSqrtAAlpha,
    };
  },
};

export function designBiquad(
  type: BiquadType,
  freqHz: number,
  sampleRate: number,
  q: number,
  gainDb = 0,
): BiquadCoeffs {
  const w0 = (2 * Math.PI * freqHz) / sampleRate;
  const cosw0 = Math.cos(w0);
  const alpha = Math.sin(w0) / (2 * Math.max(1e-6, q));
  const a = Math.pow(10, gainDb / 40);
  const raw = DESIGNERS[type]({ cosw0, alpha, a, sqrtA: Math.sqrt(a) });
  return {
    b0: raw.b0 / raw.a0,
    b1: raw.b1 / raw.a0,
    b2: raw.b2 / raw.a0,
    a1: raw.a1 / raw.a0,
    a2: raw.a2 / raw.a0,
  };
}

export interface BiquadState {
  x1: number;
  x2: number;
  y1: number;
  y2: number;
}

export function createBiquadState(): BiquadState {
  return { x1: 0, x2: 0, y1: 0, y2: 0 };
}

export function processBiquadSample(c: BiquadCoeffs, s: BiquadState, x: number): number {
  const y = c.b0 * x + c.b1 * s.x1 + c.b2 * s.x2 - c.a1 * s.y1 - c.a2 * s.y2;
  s.x2 = s.x1;
  s.x1 = x;
  s.y2 = s.y1;
  s.y1 = y;
  return y;
}

export function processBiquad(c: BiquadCoeffs, input: Float32Array): Float32Array {
  const state = createBiquadState();
  const output = new Float32Array(input.length);
  for (let i = 0; i < input.length; i += 1) {
    output[i] = processBiquadSample(c, state, input[i]!);
  }
  return output;
}

/** Magnitude of the filter's frequency response at `freqHz`. */
export function magnitudeResponse(c: BiquadCoeffs, freqHz: number, sampleRate: number): number {
  const w = (2 * Math.PI * freqHz) / sampleRate;
  const cos1 = Math.cos(w);
  const sin1 = Math.sin(w);
  const cos2 = Math.cos(2 * w);
  const sin2 = Math.sin(2 * w);
  const numRe = c.b0 + c.b1 * cos1 + c.b2 * cos2;
  const numIm = -(c.b1 * sin1 + c.b2 * sin2);
  const denRe = 1 + c.a1 * cos1 + c.a2 * cos2;
  const denIm = -(c.a1 * sin1 + c.a2 * sin2);
  const num = Math.hypot(numRe, numIm);
  const den = Math.hypot(denRe, denIm);
  return num / den;
}
