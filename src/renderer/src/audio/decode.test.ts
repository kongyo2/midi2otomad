import { describe, expect, it, vi, beforeEach } from "vitest";

const decodeAudioData = vi.hoisted(() => vi.fn());

vi.mock("./context", () => ({ getAudioContext: () => ({ decodeAudioData }) }));

import { buildWaveformPeaks, decodeAudio } from "./decode";
import type { PcmAudio } from "../../../shared/audio/mixer";

function fakeAudioBuffer(numberOfChannels: number, length: number, sampleRate = 48000): unknown {
  return {
    numberOfChannels,
    length,
    sampleRate,
    getChannelData: (c: number) => new Float32Array(length).fill(c + 1),
  };
}

beforeEach(() => {
  decodeAudioData.mockReset();
});

describe("decodeAudio", () => {
  it("decodes every channel into PCM", async () => {
    decodeAudioData.mockResolvedValueOnce(fakeAudioBuffer(2, 4, 44100));
    const pcm = await decodeAudio(new Uint8Array([0, 1, 2, 3]));
    expect(pcm.sampleRate).toBe(44100);
    expect(pcm.frames).toBe(4);
    expect(pcm.channels).toHaveLength(2);
    expect(Array.from(pcm.channels[0]!)).toEqual([1, 1, 1, 1]);
    expect(Array.from(pcm.channels[1]!)).toEqual([2, 2, 2, 2]);
  });

  it("synthesizes a silent channel when the source has none", async () => {
    decodeAudioData.mockResolvedValueOnce(fakeAudioBuffer(0, 5));
    const pcm = await decodeAudio(new Uint8Array([1]));
    expect(pcm.channels).toHaveLength(1);
    expect(pcm.channels[0]).toHaveLength(5);
    expect(pcm.frames).toBe(5);
  });
});

describe("buildWaveformPeaks", () => {
  it("downsamples a channel into magnitude buckets", () => {
    const channel = new Float32Array([0, 0.2, -0.9, 0.1, 0.5, -0.3]);
    const pcm: PcmAudio = { sampleRate: 6, channels: [channel], frames: 6 };
    const peaks = buildWaveformPeaks(pcm, 3);
    expect(peaks[0]).toBeCloseTo(0.2, 5);
    expect(peaks[1]).toBeCloseTo(0.9, 5);
    expect(peaks[2]).toBeCloseTo(0.5, 5);
  });

  it("returns zeros when there is no channel data", () => {
    const pcm: PcmAudio = { sampleRate: 48000, channels: [], frames: 0 };
    const peaks = buildWaveformPeaks(pcm, 4);
    expect(Array.from(peaks)).toEqual([0, 0, 0, 0]);
  });

  it("returns zeros when the channel is empty", () => {
    const pcm: PcmAudio = { sampleRate: 48000, channels: [new Float32Array(0)], frames: 0 };
    const peaks = buildWaveformPeaks(pcm, 2);
    expect(Array.from(peaks)).toEqual([0, 0]);
  });

  it("treats frames beyond the channel length as silence", () => {
    const pcm: PcmAudio = { sampleRate: 48000, channels: [new Float32Array([0.5, 0.5])], frames: 6 };
    const peaks = buildWaveformPeaks(pcm, 3);
    expect(peaks[0]).toBeCloseTo(0.5, 5);
    expect(peaks[1]).toBe(0);
    expect(peaks[2]).toBe(0);
  });
});
