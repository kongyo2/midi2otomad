import { describe, expect, it, vi } from "vitest";

const ffmpegVersion = vi.hoisted(() => vi.fn(() => "8.1"));

vi.mock("node-av", () => ({ ffmpegVersion }));

import { probeMedia } from "./backend";

describe("probeMedia", () => {
  it("reports the node-av backend and ffmpeg version, caching the module", async () => {
    const first = await probeMedia();
    const second = await probeMedia();
    expect(first).toEqual({ backend: "node-av", ffmpegVersion: "8.1" });
    expect(second).toEqual(first);
    expect(ffmpegVersion).toHaveBeenCalledTimes(2);
  });
});
