import { describe, expect, it } from "vitest";
import { pitchOffsetSemitones, type PitchModParams } from "./pitchmod";

function mod(overrides: Partial<PitchModParams> = {}): PitchModParams {
  return {
    glideSemitones: 0,
    glideMs: 0,
    glideCurve: 0,
    vibratoCents: 0,
    vibratoHz: 5,
    vibratoDelayMs: 0,
    vibratoFadeMs: 0,
    vibratoShape: "sine",
    ...overrides,
  };
}

describe("pitchOffsetSemitones glide", () => {
  it("glides from the initial offset down to zero", () => {
    const params = mod({ glideSemitones: 12, glideMs: 100 });
    expect(pitchOffsetSemitones(params, 0)).toBeCloseTo(12, 9);
    expect(pitchOffsetSemitones(params, 0.05)).toBeCloseTo(6, 9);
    expect(pitchOffsetSemitones(params, 0.1)).toBeCloseTo(0, 9);
    expect(pitchOffsetSemitones(params, 0.2)).toBeCloseTo(0, 9);
  });

  it("contributes nothing when the glide time is zero", () => {
    expect(pitchOffsetSemitones(mod({ glideSemitones: 12, glideMs: 0 }), 0)).toBeCloseTo(0, 9);
  });

  it("holds the glide higher for longer with a positive curve", () => {
    const value = pitchOffsetSemitones(mod({ glideSemitones: 12, glideMs: 100, glideCurve: 4 }), 0.05);
    expect(value).toBeGreaterThan(6);
  });
});

describe("pitchOffsetSemitones vibrato", () => {
  it("oscillates by the vibrato depth in semitones", () => {
    const params = mod({ vibratoCents: 100, vibratoHz: 5 });
    expect(pitchOffsetSemitones(params, 0.05)).toBeCloseTo(1, 6);
    expect(pitchOffsetSemitones(params, 0.15)).toBeCloseTo(-1, 6);
  });

  it("is silent during the vibrato delay", () => {
    expect(pitchOffsetSemitones(mod({ vibratoCents: 100, vibratoDelayMs: 100 }), 0.05)).toBe(0);
  });

  it("fades the vibrato in over the fade time", () => {
    expect(pitchOffsetSemitones(mod({ vibratoCents: 100, vibratoFadeMs: 100 }), 0.05)).toBeCloseTo(0.5, 6);
  });

  it("reaches full depth once the fade completes", () => {
    expect(pitchOffsetSemitones(mod({ vibratoCents: 100, vibratoFadeMs: 50 }), 0.15)).toBeCloseTo(-1, 6);
  });
});

describe("pitchOffsetSemitones combined", () => {
  it("sums the glide and vibrato contributions", () => {
    const params = mod({ glideSemitones: 12, glideMs: 100, vibratoCents: 100, vibratoHz: 5 });
    expect(pitchOffsetSemitones(params, 0.05)).toBeCloseTo(7, 6);
  });
});
