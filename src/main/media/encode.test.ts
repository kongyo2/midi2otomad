// @vitest-environment node
import { describe, expect, it, vi, beforeEach } from "vitest";
import { encodeWav, mp3CompatibleRate, writeExport, type PcmInput } from "./encode";

const mocks = vi.hoisted(() => {
  const frameFree = vi.fn();
  const fromAudioBuffer = vi.fn(() => ({ free: frameFree }));
  const packetFree = vi.fn();
  const writePacket = vi.fn(async () => undefined);
  const muxerClose = vi.fn(async () => undefined);
  const encoderClose = vi.fn();
  const addStream = vi.fn(() => 0);
  const encoderCreate = vi.fn(async () => ({
    packets: async function* packets(gen: AsyncIterable<unknown>) {
      for await (const frame of gen) {
        void frame;
      }
      yield { free: packetFree };
      yield null;
    },
    close: encoderClose,
  }));
  const muxerOpen = vi.fn(async () => ({ addStream, writePacket, close: muxerClose }));
  const writeFile = vi.fn(async () => undefined);
  const stat = vi.fn(async () => ({ size: 4321 }));
  return {
    frameFree,
    fromAudioBuffer,
    packetFree,
    writePacket,
    muxerClose,
    encoderClose,
    addStream,
    encoderCreate,
    muxerOpen,
    writeFile,
    stat,
  };
});

vi.mock("node:fs/promises", () => ({ writeFile: mocks.writeFile, stat: mocks.stat }));

vi.mock("node-av", () => ({
  Encoder: { create: mocks.encoderCreate },
  Muxer: { open: mocks.muxerOpen },
  Frame: { fromAudioBuffer: mocks.fromAudioBuffer },
  FF_ENCODER_LIBMP3LAME: "libmp3lame",
  AV_SAMPLE_FMT_FLTP: 8,
  AV_CHANNEL_LAYOUT_STEREO: 3,
}));

function pcm(frames: number, fill: (i: number) => number): PcmInput {
  const left = new Float32Array(frames);
  const right = new Float32Array(frames);
  for (let i = 0; i < frames; i += 1) {
    left[i] = fill(i);
    right[i] = -fill(i);
  }
  return { sampleRate: 48000, left, right, frames };
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("encodeWav", () => {
  it("writes a 24-bit PCM RIFF/WAVE file by default", () => {
    const frames = 4;
    const buffer = encodeWav(pcm(frames, (i) => [0.5, -0.5, 2, Number.NaN][i] ?? 0));
    expect(buffer.toString("ascii", 0, 4)).toBe("RIFF");
    expect(buffer.toString("ascii", 8, 12)).toBe("WAVE");
    expect(buffer.toString("ascii", 12, 16)).toBe("fmt ");
    expect(buffer.readUInt16LE(20)).toBe(1); // PCM integer
    expect(buffer.readUInt16LE(34)).toBe(24); // bit depth
    expect(buffer.byteLength).toBe(44 + frames * 6);
  });

  it("dithers and packs 16-bit PCM", () => {
    const frames = 8;
    const buffer = encodeWav(
      pcm(frames, (i) => Math.sin(i) * 1.5),
      16,
    );
    expect(buffer.readUInt16LE(20)).toBe(1);
    expect(buffer.readUInt16LE(34)).toBe(16);
    expect(buffer.byteLength).toBe(44 + frames * 4);
  });

  it("writes 32-bit IEEE float with a fact chunk", () => {
    const frames = 3;
    const buffer = encodeWav(
      pcm(frames, (i) => i / 10),
      32,
    );
    expect(buffer.readUInt16LE(20)).toBe(3); // IEEE float
    expect(buffer.readUInt16LE(34)).toBe(32);
    expect(buffer.includes(Buffer.from("fact", "ascii"))).toBe(true);
    expect(buffer.byteLength).toBe(58 + frames * 8);
  });

  it("clamps out-of-range and non-finite samples", () => {
    const buffer = encodeWav(
      { sampleRate: 48000, left: new Float32Array([2]), right: new Float32Array([-2]), frames: 1 },
      32,
    );
    const dataStart = 58;
    expect(buffer.readFloatLE(dataStart)).toBe(1);
    expect(buffer.readFloatLE(dataStart + 4)).toBe(-1);
  });

  it("treats frames beyond the channel length as silence", () => {
    const frames = 3;
    const buffer = encodeWav(
      { sampleRate: 48000, left: new Float32Array([0.5]), right: new Float32Array([0.5]), frames },
      16,
    );
    expect(buffer.byteLength).toBe(44 + frames * 4);
  });
});

describe("mp3CompatibleRate", () => {
  it("keeps a rate that MP3 already supports", () => {
    expect(mp3CompatibleRate(48000)).toBe(48000);
    expect(mp3CompatibleRate(44100)).toBe(44100);
  });

  it("halves a power-of-two multiple back into the same family", () => {
    expect(mp3CompatibleRate(96000)).toBe(48000);
    expect(mp3CompatibleRate(88200)).toBe(44100);
    expect(mp3CompatibleRate(176400)).toBe(44100);
  });

  it("folds a power-of-two multiple of a lower family back to that family", () => {
    expect(mp3CompatibleRate(64000)).toBe(32000);
  });

  it("falls back to the highest supported rate at or below an odd rate", () => {
    expect(mp3CompatibleRate(50000)).toBe(48000);
    expect(mp3CompatibleRate(144000)).toBe(48000);
  });

  it("clamps a rate below the MP3 minimum up to the lowest supported rate", () => {
    expect(mp3CompatibleRate(4000)).toBe(8000);
  });
});

describe("writeExport", () => {
  it("writes a WAV file and reports its size", async () => {
    const input = pcm(10, () => 0.25);
    const result = await writeExport(input, { format: "wav", path: "/tmp/out.wav", wavBitDepth: 16 });
    expect(mocks.writeFile).toHaveBeenCalledOnce();
    expect(mocks.writeFile.mock.calls[0]![0]).toBe("/tmp/out.wav");
    expect(result.path).toBe("/tmp/out.wav");
    expect(result.bytes).toBe(44 + 10 * 4);
    expect(result.durationSec).toBeCloseTo(10 / 48000, 9);
  });

  it("defaults to 24-bit WAV when no depth is given", async () => {
    const result = await writeExport(
      pcm(2, () => 0.1),
      { format: "wav", path: "/tmp/d.wav" },
    );
    expect(result.bytes).toBe(44 + 2 * 6);
  });

  it("encodes MP3 through node-av and reports the file size on disk", async () => {
    const frames = 2500;
    const result = await writeExport(
      pcm(frames, (i) => Math.sin(i / 20)),
      {
        format: "mp3",
        path: "/tmp/out.mp3",
        mp3Bitrate: 256,
      },
    );
    expect(mocks.encoderCreate).toHaveBeenCalledWith("libmp3lame", { bitrate: "256k" });
    expect(mocks.muxerOpen).toHaveBeenCalledWith("/tmp/out.mp3");
    expect(mocks.fromAudioBuffer).toHaveBeenCalledTimes(Math.ceil(frames / 1152));
    expect(mocks.writePacket).toHaveBeenCalledOnce();
    expect(mocks.packetFree).toHaveBeenCalledOnce();
    expect(mocks.muxerClose).toHaveBeenCalledOnce();
    expect(mocks.encoderClose).toHaveBeenCalledOnce();
    expect(mocks.stat).toHaveBeenCalledWith("/tmp/out.mp3");
    expect(result.bytes).toBe(4321);
    expect(result.durationSec).toBeCloseTo(frames / 48000, 9);
  });

  it("defaults to 320kbps MP3 when no bitrate is given", async () => {
    await writeExport(
      pcm(100, () => 0.2),
      { format: "mp3", path: "/tmp/def.mp3" },
    );
    expect(mocks.encoderCreate).toHaveBeenCalledWith("libmp3lame", { bitrate: "320k" });
  });

  it("resamples an MP3-incompatible rate down to a supported one", async () => {
    const frames = 2400;
    const left = new Float32Array(frames);
    const right = new Float32Array(frames);
    for (let i = 0; i < frames; i += 1) {
      left[i] = Math.sin(i / 20);
      right[i] = -Math.sin(i / 20);
    }
    const result = await writeExport(
      { sampleRate: 96000, left, right, frames },
      { format: "mp3", path: "/tmp/hires.mp3" },
    );
    // 2400 frames at 96 kHz become 1200 at 48 kHz: ceil(1200 / 1152) = 2 frame buffers.
    expect(mocks.fromAudioBuffer.mock.calls[0]![1]).toMatchObject({ sampleRate: 48000 });
    expect(mocks.fromAudioBuffer).toHaveBeenCalledTimes(Math.ceil(1200 / 1152));
    expect(result.durationSec).toBeCloseTo(frames / 96000, 9);
  });

  it("pads missing samples when encoding MP3 from a short buffer", async () => {
    const input = { sampleRate: 48000, left: new Float32Array(500), right: new Float32Array(500), frames: 2000 };
    const result = await writeExport(input, { format: "mp3", path: "/tmp/short.mp3" });
    expect(mocks.fromAudioBuffer).toHaveBeenCalledTimes(Math.ceil(2000 / 1152));
    expect(result.bytes).toBe(4321);
  });
});
