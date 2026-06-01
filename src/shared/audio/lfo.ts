/** Low-frequency oscillator shapes, all bipolar in `[-1, 1]` and phase-aligned. */
export type LfoShape = "sine" | "triangle" | "square" | "saw";

function wrap01(x: number): number {
  return x - Math.floor(x);
}

const SHAPES: Record<LfoShape, (frac: number) => number> = {
  sine: (frac) => Math.sin(2 * Math.PI * frac),
  triangle: (frac) => 1 - 4 * Math.abs(wrap01(frac + 0.25) - 0.5),
  square: (frac) => (frac < 0.5 ? 1 : -1),
  saw: (frac) => 2 * frac - 1,
};

/** Oscillator value for `shape` at the given `phase` in cycles (any real number). */
export function lfoValue(shape: LfoShape, phase: number): number {
  return SHAPES[shape](wrap01(phase));
}
