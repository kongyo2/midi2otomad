import { describe, expect, it } from "vitest";
import { cubicHermite } from "./interpolation";

describe("cubicHermite", () => {
  it("returns the second control point at t=0", () => {
    expect(cubicHermite(5, 10, 20, 40, 0)).toBe(10);
  });

  it("returns the third control point at t=1", () => {
    expect(cubicHermite(5, 10, 20, 40, 1)).toBe(20);
  });

  it("returns the constant when all four samples are equal", () => {
    expect(cubicHermite(0.3, 0.3, 0.3, 0.3, 0.42)).toBeCloseTo(0.3, 12);
  });

  it("reproduces a straight line exactly", () => {
    // Samples of f(x) = 2x + 1 at x = 0..3; value at the midpoint x = 1.5 is 4.
    expect(cubicHermite(1, 3, 5, 7, 0.5)).toBeCloseTo(4, 12);
  });

  it("reproduces a parabola exactly at the midpoint", () => {
    // Samples of f(x) = x^2 at x = 0..3; value at x = 1.5 is 2.25.
    expect(cubicHermite(0, 1, 4, 9, 0.5)).toBeCloseTo(2.25, 12);
  });

  it("is symmetric under reversal of the sample window", () => {
    const forward = cubicHermite(2, 7, 1, 9, 0.3);
    const reversed = cubicHermite(9, 1, 7, 2, 0.7);
    expect(forward).toBeCloseTo(reversed, 12);
  });

  it("rings beyond the inner samples near a one-sided edge", () => {
    // Inner samples are both 0, yet the rising outer sample pulls the curve
    // below zero — ringing that a clamped linear interpolator can never produce.
    const value = cubicHermite(0, 0, 0, 1, 0.5);
    expect(value).toBeLessThan(0);
  });
});
