import { describe, expect, it, vi, beforeEach } from "vitest";
import type { Sample } from "../../../shared/schemas/project";
import type { PcmAudio } from "../../../shared/audio/mixer";

const ctx = vi.hoisted(() => {
  const makeSource = () => ({
    buffer: null as unknown,
    playbackRate: { value: 0 },
    loop: false,
    loopStart: 0,
    loopEnd: 0,
    connect: vi.fn(),
    start: vi.fn(),
    stop: vi.fn(),
  });
  const makeGain = () => ({
    gain: { setValueAtTime: vi.fn(), linearRampToValueAtTime: vi.fn() },
    connect: vi.fn(),
  });
  return {
    currentTime: 10,
    destination: { id: "destination" },
    createBuffer: vi.fn(() => ({ copyToChannel: vi.fn() })),
    createBufferSource: vi.fn(makeSource),
    createGain: vi.fn(makeGain),
  };
});

vi.mock("./context", () => ({ getAudioContext: () => ctx, resumeAudioContext: vi.fn(async () => undefined) }));

import { previewSample } from "./preview";

function sample(over: Partial<Sample> = {}): Sample {
  return {
    id: "s1",
    name: "s",
    fileName: "",
    basePitch: 60,
    tuneCents: 0,
    gain: 0.8,
    durationSec: 1,
    loop: { enabled: false, startSec: 0, endSec: 0 },
    envelope: { attackMs: 5, releaseMs: 120 },
    ...over,
  };
}

const pcm: PcmAudio = { sampleRate: 1000, channels: [new Float32Array(1000), new Float32Array(1000)], frames: 1000 };

function lastSource(): ReturnType<typeof ctx.createBufferSource> {
  const results = ctx.createBufferSource.mock.results;
  return results[results.length - 1]!.value;
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("previewSample", () => {
  it("plays a looped one-shot at an explicit pitch", () => {
    previewSample(pcm, sample({ loop: { enabled: true, startSec: 0.1, endSec: 0.5 } }), 72);
    expect(ctx.createBuffer).toHaveBeenCalledWith(2, 1000, 1000);
    const source = lastSource();
    expect(source.loop).toBe(true);
    expect(source.loopStart).toBe(0.1);
    expect(source.loopEnd).toBe(0.5);
    expect(source.playbackRate.value).toBeCloseTo(2, 9);
    expect(source.start).toHaveBeenCalledWith(10);
    expect(source.stop).toHaveBeenCalled();
  });

  it("loops over the full sample when the loop end is not past the start", () => {
    previewSample(pcm, sample({ loop: { enabled: true, startSec: 0.5, endSec: 0.1 } }));
    const source = lastSource();
    expect(source.loop).toBe(true);
    expect(source.loopEnd).toBe(1);
    expect(source.playbackRate.value).toBeCloseTo(1, 9);
  });

  it("plays a non-looping sample at its base pitch", () => {
    previewSample(pcm, sample({ loop: { enabled: false, startSec: 0, endSec: 0 } }));
    const source = lastSource();
    expect(source.loop).toBe(false);
    expect(source.start).toHaveBeenCalledWith(10);
    expect(source.connect).toHaveBeenCalled();
  });
});
