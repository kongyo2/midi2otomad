import { describe, expect, it } from "vitest";
import type {
  BouncePcm,
  BounceRequest,
  BounceResponse,
  ExportFormat,
  LoadedFile,
  MediaProbe,
  WavBitDepth,
} from "./media";

describe("media shared types", () => {
  it("describes a media probe result", () => {
    const probe: MediaProbe = { backend: "node-av", ffmpegVersion: "8.1" };
    expect(probe.backend).toBe("node-av");
  });

  it("supports the supported export formats and bit depths", () => {
    const formats: ExportFormat[] = ["wav", "mp3"];
    const depths: WavBitDepth[] = [16, 24, 32];
    expect(formats).toHaveLength(2);
    expect(depths).toContain(24);
  });

  it("models a bounce request and its pcm payload", () => {
    const pcm: BouncePcm = {
      sampleRate: 48000,
      left: new Float32Array(4),
      right: new Float32Array(4),
      frames: 4,
    };
    const request: BounceRequest = { format: "wav", pcm, defaultName: "song", wavBitDepth: 24 };
    expect(request.pcm.frames).toBe(4);
  });

  it("models both bounce response variants", () => {
    const ok: BounceResponse = { ok: true, path: "/tmp/a.wav", bytes: 100, durationSec: 1 };
    const failed: BounceResponse = { ok: false, canceled: true };
    expect(ok.ok && ok.bytes).toBe(100);
    expect(failed.ok === false && failed.canceled).toBe(true);
  });

  it("models a loaded file", () => {
    const file: LoadedFile = { name: "demo.mid", data: new Uint8Array([1, 2, 3]) };
    expect(file.data).toHaveLength(3);
  });
});
