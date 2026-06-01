import { describe, expect, it } from "vitest";
import { lfoValue, type LfoShape } from "./lfo";

describe("lfoValue sine", () => {
  it("traces a sine through its quarter-phase landmarks", () => {
    expect(lfoValue("sine", 0)).toBeCloseTo(0, 9);
    expect(lfoValue("sine", 0.25)).toBeCloseTo(1, 9);
    expect(lfoValue("sine", 0.5)).toBeCloseTo(0, 9);
    expect(lfoValue("sine", 0.75)).toBeCloseTo(-1, 9);
  });
});

describe("lfoValue triangle", () => {
  it("hits the same landmarks as the sine but moves linearly", () => {
    expect(lfoValue("triangle", 0)).toBeCloseTo(0, 9);
    expect(lfoValue("triangle", 0.25)).toBeCloseTo(1, 9);
    expect(lfoValue("triangle", 0.5)).toBeCloseTo(0, 9);
    expect(lfoValue("triangle", 0.75)).toBeCloseTo(-1, 9);
    expect(lfoValue("triangle", 0.125)).toBeCloseTo(0.5, 9);
  });
});

describe("lfoValue square", () => {
  it("is high on the first half of the cycle and low on the second", () => {
    expect(lfoValue("square", 0.25)).toBe(1);
    expect(lfoValue("square", 0.75)).toBe(-1);
  });
});

describe("lfoValue saw", () => {
  it("ramps from -1 up to +1 across the cycle", () => {
    expect(lfoValue("saw", 0)).toBeCloseTo(-1, 9);
    expect(lfoValue("saw", 0.5)).toBeCloseTo(0, 9);
    expect(lfoValue("saw", 0.999)).toBeGreaterThan(0.99);
  });
});

describe("lfoValue phase wrapping", () => {
  it("wraps phases at or beyond one cycle", () => {
    expect(lfoValue("sine", 1.25)).toBeCloseTo(lfoValue("sine", 0.25), 9);
  });

  it("wraps negative phases forward", () => {
    expect(lfoValue("saw", -0.5)).toBeCloseTo(lfoValue("saw", 0.5), 9);
  });

  it("stays within the unit range for every shape", () => {
    const shapes: LfoShape[] = ["sine", "triangle", "square", "saw"];
    for (const shape of shapes) {
      for (let i = 0; i < 64; i += 1) {
        const value = lfoValue(shape, i / 64);
        expect(value).toBeGreaterThanOrEqual(-1);
        expect(value).toBeLessThanOrEqual(1);
      }
    }
  });
});
