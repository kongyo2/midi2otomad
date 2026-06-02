import { describe, expect, it } from "vitest";
import { envelopeLevel, type EnvelopeParams } from "./envelope";

function env(overrides: Partial<EnvelopeParams> = {}): EnvelopeParams {
  return {
    delayMs: 0,
    attackMs: 100,
    attackCurve: 0,
    holdMs: 0,
    decayMs: 0,
    decayCurve: 0,
    sustain: 1,
    releaseMs: 100,
    releaseCurve: 0,
    ...overrides,
  };
}

const HELD_OPEN = 1000;

describe("envelopeLevel pre-release stages", () => {
  it("is silent throughout the delay stage", () => {
    expect(envelopeLevel(env({ delayMs: 50 }), 0.02, HELD_OPEN)).toBe(0);
  });

  it("rises linearly through a zero-curve attack", () => {
    expect(envelopeLevel(env(), 0.05, HELD_OPEN)).toBeCloseTo(0.5, 9);
  });

  it("reaches unity at the end of the attack", () => {
    expect(envelopeLevel(env(), 0.1, HELD_OPEN)).toBeCloseTo(1, 9);
  });

  it("holds at unity during the hold stage", () => {
    expect(envelopeLevel(env({ holdMs: 100, sustain: 0.5 }), 0.15, HELD_OPEN)).toBeCloseTo(1, 9);
  });

  it("decays toward the sustain level after the hold", () => {
    const value = envelopeLevel(env({ decayMs: 100, sustain: 0.4 }), 0.15, HELD_OPEN);
    expect(value).toBeCloseTo(0.7, 9);
  });

  it("settles at the sustain level when held open past the decay", () => {
    expect(envelopeLevel(env({ decayMs: 100, sustain: 0.3 }), 5, HELD_OPEN)).toBeCloseTo(0.3, 9);
  });
});

describe("envelopeLevel release stage", () => {
  it("releases from the sustain level to zero after note-off", () => {
    const e = env({ releaseMs: 100 });
    expect(envelopeLevel(e, 1, 1)).toBeCloseTo(1, 9);
    expect(envelopeLevel(e, 1.05, 1)).toBeCloseTo(0.5, 9);
  });

  it("starts the release from the attack level for a note cut short", () => {
    const e = env({ attackMs: 200, releaseMs: 100 });
    expect(envelopeLevel(e, 0.1, 0.1)).toBeCloseTo(0.5, 9);
    expect(envelopeLevel(e, 0.15, 0.1)).toBeCloseTo(0.25, 9);
  });

  it("is silent once the release has fully elapsed", () => {
    expect(envelopeLevel(env({ releaseMs: 100 }), 1.5, 1)).toBe(0);
  });
});

describe("envelopeLevel degenerate stages", () => {
  it("jumps to the peak immediately for a zero-length attack", () => {
    expect(envelopeLevel(env({ attackMs: 0, holdMs: 50, sustain: 0.5 }), 0, HELD_OPEN)).toBeCloseTo(1, 9);
  });

  it("drops to silence immediately for a zero-length release", () => {
    expect(envelopeLevel(env({ releaseMs: 0 }), 0.5, 0.5)).toBe(0);
  });
});

describe("envelopeLevel curve shaping", () => {
  it("bends a positive attack curve below the linear ramp", () => {
    expect(envelopeLevel(env({ attackCurve: 4 }), 0.05, HELD_OPEN)).toBeLessThan(0.5);
  });
});
