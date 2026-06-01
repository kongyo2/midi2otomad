import { describe, expect, it, vi, beforeEach } from "vitest";
import { SampleSchema } from "../../../shared/schemas/project";
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
    gain: { setValueAtTime: vi.fn(), linearRampToValueAtTime: vi.fn(), setValueCurveAtTime: vi.fn() },
    connect: vi.fn(),
  });
  const makeFilter = () => ({
    type: "",
    frequency: { value: 0 },
    Q: { value: 0 },
    gain: { value: 0 },
    connect: vi.fn(),
  });
  return {
    currentTime: 10,
    destination: { id: "destination" },
    createBuffer: vi.fn(() => ({ copyToChannel: vi.fn() })),
    createBufferSource: vi.fn(makeSource),
    createGain: vi.fn(makeGain),
    createBiquadFilter: vi.fn(makeFilter),
  };
});

vi.mock("./context", () => ({ getAudioContext: () => ctx, resumeAudioContext: vi.fn(async () => undefined) }));

import { previewSample } from "./preview";

function sample(over: Record<string, unknown> = {}): ReturnType<typeof SampleSchema.parse> {
  return SampleSchema.parse({ id: "s1", name: "s", gain: 0.8, durationSec: 1, ...over });
}

const pcm: PcmAudio = { sampleRate: 1000, channels: [new Float32Array(1000), new Float32Array(1000)], frames: 1000 };

function lastSource(): ReturnType<typeof ctx.createBufferSource> {
  const results = ctx.createBufferSource.mock.results;
  return results[results.length - 1]!.value;
}

function lastGain(): ReturnType<typeof ctx.createGain> {
  const results = ctx.createGain.mock.results;
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

  it("shapes the preview gain toward the sustain level", () => {
    previewSample(pcm, sample({ gain: 0.8, envelope: { attackMs: 5, decayMs: 50, sustain: 0.5, releaseMs: 100 } }));
    const curve = lastGain().gain.setValueCurveAtTime.mock.calls.at(-1)![0] as Float32Array;
    expect(Math.max(...curve)).toBeCloseTo(0.8, 2);
    expect(curve[curve.length - 1]).toBeCloseTo(0, 5);
    expect(Array.from(curve).some((v) => Math.abs(v - 0.4) < 0.02)).toBe(true);
  });

  it("honors the envelope curve shape in the preview", () => {
    previewSample(pcm, sample({ envelope: { attackMs: 200, attackCurve: 0, releaseMs: 50 } }));
    previewSample(pcm, sample({ envelope: { attackMs: 200, attackCurve: 5, releaseMs: 50 } }));
    const results = ctx.createGain.mock.results;
    const linear = results[0]!.value.gain.setValueCurveAtTime.mock.calls[0]![0] as Float32Array;
    const shaped = results[1]!.value.gain.setValueCurveAtTime.mock.calls[0]![0] as Float32Array;
    let differs = false;
    for (let i = 0; i < Math.min(linear.length, shaped.length); i += 1) {
      if (Math.abs(linear[i]! - shaped[i]!) > 1e-3) {
        differs = true;
        break;
      }
    }
    expect(differs).toBe(true);
  });

  it("routes through a biquad filter when the sample filter is enabled", () => {
    previewSample(pcm, sample({ filter: { enabled: true, type: "highpass", cutoffHz: 800, q: 2, gainDb: 3 } }));
    expect(ctx.createBiquadFilter).toHaveBeenCalledTimes(1);
    const filter = ctx.createBiquadFilter.mock.results.at(-1)!.value;
    expect(filter.type).toBe("highpass");
    expect(filter.frequency.value).toBe(800);
    expect(filter.Q.value).toBe(2);
    expect(lastSource().connect).toHaveBeenCalledWith(filter);
  });
});
