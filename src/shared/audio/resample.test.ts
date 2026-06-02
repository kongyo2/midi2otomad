import { describe, expect, it } from "vitest";
import { resampleChannel } from "./resample";

function tone(freqHz: number, sampleRate: number, frames: number): Float32Array {
  const ch = new Float32Array(frames);
  for (let i = 0; i < frames; i += 1) {
    ch[i] = Math.sin((2 * Math.PI * freqHz * i) / sampleRate);
  }
  return ch;
}

function rms(arr: Float32Array, start = 0): number {
  let sum = 0;
  for (let i = start; i < arr.length; i += 1) {
    sum += arr[i]! * arr[i]!;
  }
  return Math.sqrt(sum / Math.max(1, arr.length - start));
}

function allFinite(arr: Float32Array): boolean {
  for (let i = 0; i < arr.length; i += 1) {
    if (!Number.isFinite(arr[i])) {
      return false;
    }
  }
  return true;
}

describe("resampleChannel", () => {
  it("halves the length when dropping to half the sample rate", () => {
    const out = resampleChannel(new Float32Array(100), 48000, 24000);
    expect(out.length).toBe(50);
  });

  it("doubles the length when doubling the sample rate", () => {
    const out = resampleChannel(new Float32Array(50), 24000, 48000);
    expect(out.length).toBe(100);
  });

  it("returns the same samples when the rate is unchanged", () => {
    const input = tone(1000, 48000, 64);
    const out = resampleChannel(input, 48000, 48000);
    expect(out.length).toBe(64);
    for (let i = 0; i < input.length; i += 1) {
      expect(out[i]).toBeCloseTo(input[i]!, 5);
    }
  });

  it("anti-aliases content above the destination Nyquist when downsampling", () => {
    const src = 48000;
    const dst = 24000;
    const frames = 4800;
    const high = resampleChannel(tone(18000, src, frames), src, dst);
    const low = resampleChannel(tone(1000, src, frames), src, dst);
    expect(rms(high, high.length / 2)).toBeLessThan(0.2);
    expect(rms(low, low.length / 2)).toBeGreaterThan(0.45);
  });

  it("keeps every output sample finite", () => {
    const out = resampleChannel(tone(5000, 96000, 960), 96000, 48000);
    expect(allFinite(out)).toBe(true);
  });
});
