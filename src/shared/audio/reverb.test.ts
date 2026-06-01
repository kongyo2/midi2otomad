import { describe, expect, it } from "vitest";
import { createReverb, type ReverbParams } from "./reverb";

const FS = 48000;

function rev(overrides: Partial<ReverbParams> = {}): ReverbParams {
  return { roomSize: 0.5, damping: 0.5, width: 1, wet: 1, dry: 0, preDelayMs: 0, ...overrides };
}

function impulse(n: number): Float32Array {
  const buffer = new Float32Array(n);
  buffer[0] = 1;
  return buffer;
}

function energy(arr: Float32Array, start: number, end: number): number {
  let sum = 0;
  for (let i = start; i < end; i += 1) {
    sum += arr[i]! * arr[i]!;
  }
  return sum;
}

function firstAudible(arr: Float32Array, threshold: number): number {
  for (let i = 0; i < arr.length; i += 1) {
    if (Math.abs(arr[i]!) > threshold) {
      return i;
    }
  }
  return arr.length;
}

describe("createReverb tail", () => {
  it("produces a decaying tail after an impulse", () => {
    const out = createReverb(FS, rev()).processBlock(impulse(16000), impulse(16000));
    expect(energy(out.left, 1200, 16000)).toBeGreaterThan(0);
    expect(energy(out.left, 8000, 16000)).toBeLessThan(energy(out.left, 0, 8000));
  });

  it("rings longer for a larger room size", () => {
    const small = createReverb(FS, rev({ roomSize: 0.3 })).processBlock(impulse(16000), impulse(16000));
    const large = createReverb(FS, rev({ roomSize: 0.9 })).processBlock(impulse(16000), impulse(16000));
    expect(energy(large.left, 8000, 16000)).toBeGreaterThan(energy(small.left, 8000, 16000));
  });

  it("decays faster as damping increases", () => {
    const bright = createReverb(FS, rev({ damping: 0 })).processBlock(impulse(16000), impulse(16000));
    const dark = createReverb(FS, rev({ damping: 1 })).processBlock(impulse(16000), impulse(16000));
    expect(energy(dark.left, 0, 16000)).toBeLessThan(energy(bright.left, 0, 16000));
  });
});

describe("createReverb mixing", () => {
  it("passes the dry signal straight through when fully dry", () => {
    const out = createReverb(FS, rev({ wet: 0, dry: 1 })).processBlock(impulse(100), impulse(100));
    expect(out.left[0]).toBeCloseTo(1, 9);
    expect(out.left[50]).toBeCloseTo(0, 9);
  });

  it("collapses to mono at zero width and decorrelates at full width", () => {
    const mono = createReverb(FS, rev({ width: 0 })).processBlock(impulse(8000), impulse(8000));
    for (const i of [2000, 4000, 6000]) {
      expect(mono.left[i]).toBeCloseTo(mono.right[i]!, 12);
    }
    const wide = createReverb(FS, rev({ width: 1 })).processBlock(impulse(8000), impulse(8000));
    let differs = false;
    for (let i = 1200; i < 8000; i += 1) {
      if (Math.abs(wide.left[i]! - wide.right[i]!) > 1e-9) {
        differs = true;
        break;
      }
    }
    expect(differs).toBe(true);
  });

  it("delays the onset of the wet tail with a pre-delay", () => {
    const direct = createReverb(FS, rev()).processBlock(impulse(16000), impulse(16000));
    const delayed = createReverb(FS, rev({ preDelayMs: 50 })).processBlock(impulse(16000), impulse(16000));
    expect(firstAudible(delayed.left, 1e-4)).toBeGreaterThan(firstAudible(direct.left, 1e-4) + 1000);
  });
});

describe("createReverb stability", () => {
  it("keeps the output finite and bounded", () => {
    const out = createReverb(FS, rev({ roomSize: 0.95 })).processBlock(impulse(16000), impulse(16000));
    for (let i = 0; i < out.left.length; i += 1) {
      expect(Number.isFinite(out.left[i])).toBe(true);
      expect(Math.abs(out.left[i]!)).toBeLessThan(1);
    }
  });
});
