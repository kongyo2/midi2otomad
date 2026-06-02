import { describe, expect, it } from "vitest";
import { createBiquadState, designBiquad, magnitudeResponse, processBiquad, processBiquadSample } from "./filter";

const FS = 48000;

function magAt(type: Parameters<typeof designBiquad>[0], freq: number, q = 0.707, gainDb = 0): number {
  const coeffs = designBiquad(type, 1000, FS, q, gainDb);
  return magnitudeResponse(coeffs, freq, FS);
}

describe("designBiquad lowpass", () => {
  it("passes DC at unity gain", () => {
    expect(magAt("lowpass", 0)).toBeCloseTo(1, 6);
  });

  it("strongly attenuates content far above the cutoff", () => {
    expect(magAt("lowpass", 20000)).toBeLessThan(0.05);
  });
});

describe("designBiquad highpass", () => {
  it("rejects DC entirely", () => {
    expect(magAt("highpass", 0)).toBeCloseTo(0, 6);
  });

  it("passes content well above the cutoff", () => {
    expect(magAt("highpass", 20000)).toBeGreaterThan(0.9);
  });
});

describe("designBiquad bandpass", () => {
  it("peaks near the centre and rejects DC and Nyquist", () => {
    const center = magAt("bandpass", 1000);
    expect(center).toBeGreaterThan(magAt("bandpass", 100));
    expect(center).toBeGreaterThan(magAt("bandpass", 10000));
    expect(magAt("bandpass", 0)).toBeCloseTo(0, 6);
  });
});

describe("designBiquad notch", () => {
  it("nulls the centre frequency while passing the rest", () => {
    expect(magAt("notch", 1000)).toBeLessThan(0.01);
    expect(magAt("notch", 0)).toBeCloseTo(1, 6);
  });
});

describe("designBiquad peaking", () => {
  it("boosts the centre frequency by the requested gain", () => {
    expect(magAt("peaking", 1000, 1, 12)).toBeCloseTo(Math.pow(10, 12 / 20), 2);
    expect(magAt("peaking", 0, 1, 12)).toBeCloseTo(1, 4);
  });

  it("cuts the centre frequency for negative gain", () => {
    expect(magAt("peaking", 1000, 1, -12)).toBeLessThan(1);
  });
});

describe("designBiquad shelves", () => {
  it("lifts low frequencies with a low shelf", () => {
    expect(magAt("lowshelf", 0, 0.707, 12)).toBeCloseTo(Math.pow(10, 12 / 20), 2);
    expect(magAt("lowshelf", 23000, 0.707, 12)).toBeCloseTo(1, 1);
  });

  it("lifts high frequencies with a high shelf", () => {
    expect(magAt("highshelf", 23000, 0.707, 12)).toBeCloseTo(Math.pow(10, 12 / 20), 1);
    expect(magAt("highshelf", 0, 0.707, 12)).toBeCloseTo(1, 4);
  });
});

describe("designBiquad allpass", () => {
  it("keeps unity magnitude across the spectrum", () => {
    expect(magAt("allpass", 100)).toBeCloseTo(1, 6);
    expect(magAt("allpass", 1000)).toBeCloseTo(1, 6);
    expect(magAt("allpass", 10000)).toBeCloseTo(1, 6);
  });
});

describe("processBiquad", () => {
  it("settles a constant input to its DC gain for a lowpass", () => {
    const coeffs = designBiquad("lowpass", 1000, FS, 0.707);
    const input = new Float32Array(2000).fill(1);
    const output = processBiquad(coeffs, input);
    expect(output[output.length - 1]).toBeCloseTo(1, 4);
  });

  it("attenuates a Nyquist-rate signal through a lowpass and stays finite", () => {
    const coeffs = designBiquad("lowpass", 1000, FS, 0.707);
    const input = new Float32Array(512);
    for (let i = 0; i < input.length; i += 1) {
      input[i] = i % 2 === 0 ? 1 : -1;
    }
    const output = processBiquad(coeffs, input);
    let peak = 0;
    for (let i = 256; i < output.length; i += 1) {
      peak = Math.max(peak, Math.abs(output[i]!));
      expect(Number.isFinite(output[i])).toBe(true);
    }
    expect(peak).toBeLessThan(0.1);
  });

  it("matches a manual per-sample run", () => {
    const coeffs = designBiquad("highpass", 2000, FS, 1.2);
    const input = Float32Array.from({ length: 32 }, (_value, i) => Math.sin(i));
    const expected = new Float32Array(input.length);
    const state = createBiquadState();
    for (let i = 0; i < input.length; i += 1) {
      expected[i] = processBiquadSample(coeffs, state, input[i]!);
    }
    expect(Array.from(processBiquad(coeffs, input))).toEqual(Array.from(expected));
  });
});
