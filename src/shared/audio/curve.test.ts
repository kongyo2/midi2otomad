import { describe, expect, it } from "vitest";
import { shapeCurve } from "./curve";

describe("shapeCurve", () => {
  it("is the identity at zero tension", () => {
    expect(shapeCurve(0.25, 0)).toBeCloseTo(0.25, 12);
    expect(shapeCurve(0.5, 0)).toBeCloseTo(0.5, 12);
    expect(shapeCurve(0.8, 0)).toBeCloseTo(0.8, 12);
  });

  it("pins both endpoints regardless of tension", () => {
    expect(shapeCurve(0, 5)).toBeCloseTo(0, 12);
    expect(shapeCurve(1, 5)).toBeCloseTo(1, 12);
    expect(shapeCurve(0, -5)).toBeCloseTo(0, 12);
    expect(shapeCurve(1, -5)).toBeCloseTo(1, 12);
  });

  it("bends below the diagonal for positive tension (ease-in)", () => {
    expect(shapeCurve(0.5, 3)).toBeLessThan(0.5);
  });

  it("bends above the diagonal for negative tension (ease-out)", () => {
    expect(shapeCurve(0.5, -3)).toBeGreaterThan(0.5);
  });

  it("stays monotonically increasing across the segment", () => {
    let previous = -Infinity;
    for (let i = 0; i <= 20; i += 1) {
      const value = shapeCurve(i / 20, 4);
      expect(value).toBeGreaterThan(previous);
      previous = value;
    }
  });

  it("is point-symmetric about the centre under tension negation", () => {
    for (const x of [0.1, 0.37, 0.62, 0.95]) {
      expect(shapeCurve(x, 2.5) + shapeCurve(1 - x, -2.5)).toBeCloseTo(1, 12);
    }
  });

  it("clamps progress below zero and above one", () => {
    expect(shapeCurve(-0.5, 3)).toBe(0);
    expect(shapeCurve(1.5, 3)).toBe(1);
  });
});
