import { describe, expect, it, vi, beforeEach } from "vitest";
import { SampleSchema } from "../../../shared/schemas/project";
import type { PcmAudio } from "../../../shared/audio/mixer";

const ctx = vi.hoisted(() => {
  const makeSource = () => ({ buffer: null as unknown, connect: vi.fn(), start: vi.fn() });
  return {
    sampleRate: 8000,
    destination: { id: "destination" },
    createBuffer: vi.fn((_channels: number, _frames: number, _rate: number) => ({ copyToChannel: vi.fn() })),
    createBufferSource: vi.fn(makeSource),
  };
});

vi.mock("./context", () => ({ getAudioContext: () => ctx, resumeAudioContext: vi.fn(async () => undefined) }));

import { previewSample } from "./preview";

function sample(over: Record<string, unknown> = {}): ReturnType<typeof SampleSchema.parse> {
  return SampleSchema.parse({ id: "s1", name: "s", basePitch: 60, gain: 1, durationSec: 1, ...over });
}

function rampPcm(frames = 1000): PcmAudio {
  const ch = new Float32Array(frames);
  for (let i = 0; i < frames; i += 1) {
    ch[i] = (i % 200) / 200;
  }
  return { sampleRate: 1000, channels: [ch], frames };
}

function brightPcm(frames = 1000): PcmAudio {
  const ch = new Float32Array(frames);
  for (let i = 0; i < frames; i += 1) {
    ch[i] = Math.floor(i / 2) % 2 === 0 ? 1 : -1;
  }
  return { sampleRate: 1000, channels: [ch], frames };
}

function lastLeft(): Float32Array {
  const buffer = ctx.createBuffer.mock.results.at(-1)!.value;
  return buffer.copyToChannel.mock.calls[0]![0] as Float32Array;
}

function peakOf(data: Float32Array): number {
  let peak = 0;
  for (const value of data) {
    peak = Math.max(peak, Math.abs(value));
  }
  return peak;
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("previewSample", () => {
  it("renders the sample through the engine into a stereo buffer and plays it", () => {
    previewSample(rampPcm(), sample());
    const call = ctx.createBuffer.mock.calls.at(-1)!;
    expect(call[0]).toBe(2);
    expect(call[1]).toBeGreaterThan(0);
    expect(call[2]).toBe(ctx.sampleRate);
    const buffer = ctx.createBuffer.mock.results.at(-1)!.value;
    expect(buffer.copyToChannel).toHaveBeenCalledTimes(2);
    expect(peakOf(lastLeft())).toBeGreaterThan(0);
    const source = ctx.createBufferSource.mock.results.at(-1)!.value;
    expect(source.connect).toHaveBeenCalledWith(ctx.destination);
    expect(source.start).toHaveBeenCalled();
  });

  it("auditions the requested pitch rather than the base pitch", () => {
    previewSample(rampPcm(), sample(), 60);
    const atBase = Float32Array.from(lastLeft());
    previewSample(rampPcm(), sample(), 72);
    const anOctaveUp = lastLeft();
    let differs = false;
    for (let i = 0; i < Math.min(atBase.length, anOctaveUp.length); i += 1) {
      if (Math.abs(atBase[i]! - anOctaveUp[i]!) > 1e-4) {
        differs = true;
        break;
      }
    }
    expect(differs).toBe(true);
  });

  it("plays a longer audition for a looped sample", () => {
    previewSample(brightPcm(), sample({ loop: { enabled: false, startSec: 0, endSec: 0 } }));
    const dryFrames = ctx.createBuffer.mock.calls.at(-1)![1];
    previewSample(brightPcm(), sample({ loop: { enabled: true, startSec: 0, endSec: 0.5 } }));
    const loopFrames = ctx.createBuffer.mock.calls.at(-1)![1];
    expect(loopFrames).toBeGreaterThan(dryFrames);
  });

  it("applies the sample filter so the preview matches the mix", () => {
    previewSample(brightPcm(), sample());
    const openPeak = peakOf(lastLeft());
    previewSample(brightPcm(), sample({ filter: { enabled: true, type: "lowpass", cutoffHz: 200 } }));
    const filteredPeak = peakOf(lastLeft());
    expect(filteredPeak).toBeLessThan(openPeak);
  });
});
