import { describe, expect, it, vi, beforeEach } from "vitest";
import type { MixResult } from "../../../shared/audio/mixer";

interface FakeSource {
  buffer: unknown;
  connect: ReturnType<typeof vi.fn>;
  start: ReturnType<typeof vi.fn>;
  stop: ReturnType<typeof vi.fn>;
  disconnect: ReturnType<typeof vi.fn>;
  onended: null | (() => void);
}

const env = vi.hoisted(() => {
  const sources: FakeSource[] = [];
  const ctx = {
    currentTime: 0,
    destination: { id: "destination" },
    createGain: vi.fn(() => ({ connect: vi.fn() })),
    createAnalyser: vi.fn(() => ({ fftSize: 0, connect: vi.fn() })),
    createBuffer: vi.fn((channels: number, frames: number, sampleRate: number) => ({
      duration: frames / sampleRate,
      copyToChannel: vi.fn(),
    })),
    createBufferSource: vi.fn((): FakeSource => {
      const source: FakeSource = {
        buffer: null,
        connect: vi.fn(),
        start: vi.fn(),
        stop: vi.fn(),
        disconnect: vi.fn(),
        onended: null,
      };
      sources.push(source);
      return source;
    }),
  };
  return { ctx, sources, resumeAudioContext: vi.fn(async () => undefined) };
});

vi.mock("./context", () => ({ getAudioContext: () => env.ctx, resumeAudioContext: env.resumeAudioContext }));

import { PreviewEngine } from "./engine";

function makeMix(frames = 1000, sampleRate = 1000): MixResult {
  return {
    sampleRate,
    left: new Float32Array(frames),
    right: new Float32Array(frames),
    frames,
    durationSec: frames / sampleRate,
    peak: 0.5,
  };
}

function lastSource(): FakeSource {
  return env.sources[env.sources.length - 1]!;
}

function setStopping(engine: PreviewEngine, value: boolean): void {
  (engine as unknown as { stopping: boolean }).stopping = value;
}

beforeEach(() => {
  env.ctx.currentTime = 0;
  env.sources.length = 0;
  vi.clearAllMocks();
});

describe("PreviewEngine", () => {
  it("starts stopped with an analyser and no buffer", () => {
    const engine = new PreviewEngine();
    expect(engine.transport).toBe("stopped");
    expect(engine.durationSec).toBe(0);
    expect(engine.getMasterAnalyser().fftSize).toBe(1024);
  });

  it("loads a mix while stopped and reports its duration", () => {
    const engine = new PreviewEngine();
    engine.setMix(makeMix(2000, 1000));
    expect(engine.durationSec).toBe(2);
    expect(engine.getPosition()).toBe(0);
    expect(env.ctx.createBuffer).toHaveBeenCalledWith(2, 2000, 1000);
    const buffer = env.ctx.createBuffer.mock.results[0]!.value as { copyToChannel: ReturnType<typeof vi.fn> };
    expect(buffer.copyToChannel).toHaveBeenCalledTimes(2);
  });

  it("guarantees at least one frame in the playback buffer", () => {
    const engine = new PreviewEngine();
    engine.setMix(makeMix(0, 1000));
    expect(env.ctx.createBuffer).toHaveBeenCalledWith(2, 1, 1000);
  });

  it("plays from the current offset and tracks position", () => {
    const engine = new PreviewEngine();
    engine.setMix(makeMix(1000, 1000));
    env.ctx.currentTime = 5;
    engine.play();
    expect(env.resumeAudioContext).toHaveBeenCalled();
    expect(engine.transport).toBe("playing");
    const source = lastSource();
    expect(source.buffer).not.toBeNull();
    expect(source.connect).toHaveBeenCalled();
    expect(source.start).toHaveBeenCalledWith(0, 0);

    env.ctx.currentTime = 5.5;
    expect(engine.getPosition()).toBeCloseTo(0.5, 9);
    env.ctx.currentTime = 7;
    expect(engine.getPosition()).toBe(1);
  });

  it("captures the play position when pausing", () => {
    const engine = new PreviewEngine();
    engine.setMix(makeMix(1000, 1000));
    env.ctx.currentTime = 5;
    engine.play();
    env.ctx.currentTime = 5.5;
    engine.pause();
    expect(engine.transport).toBe("paused");
    expect(engine.getPosition()).toBeCloseTo(0.5, 9);
  });

  it("ignores pause when not playing", () => {
    const engine = new PreviewEngine();
    engine.setMix(makeMix());
    engine.pause();
    expect(engine.transport).toBe("stopped");
  });

  it("stops, tears down the source and rewinds", () => {
    const engine = new PreviewEngine();
    engine.setMix(makeMix());
    engine.play();
    const source = lastSource();
    engine.stop();
    expect(engine.transport).toBe("stopped");
    expect(engine.getPosition()).toBe(0);
    expect(source.stop).toHaveBeenCalled();
    expect(source.disconnect).toHaveBeenCalled();
  });

  it("re-renders during playback and resumes at the same position", () => {
    const engine = new PreviewEngine();
    engine.setMix(makeMix(1000, 1000));
    env.ctx.currentTime = 2;
    engine.play();
    env.ctx.currentTime = 2.3;
    engine.setMix(makeMix(1000, 1000));
    expect(engine.transport).toBe("playing");
    expect(lastSource().start.mock.calls.at(-1)).toEqual([0, expect.closeTo(0.3, 6)]);
  });

  it("tolerates a source that throws while being torn down", () => {
    const engine = new PreviewEngine();
    engine.setMix(makeMix());
    engine.play();
    lastSource().stop.mockImplementation(() => {
      throw new Error("already stopped");
    });
    expect(() => {
      engine.stop();
    }).not.toThrow();
    expect(engine.transport).toBe("stopped");
  });

  it("resets and notifies when playback ends naturally", () => {
    const engine = new PreviewEngine();
    const onEnded = vi.fn();
    engine.onEnded = onEnded;
    engine.setMix(makeMix());
    engine.play();
    lastSource().onended?.();
    expect(engine.transport).toBe("stopped");
    expect(engine.getPosition()).toBe(0);
    expect(onEnded).toHaveBeenCalledTimes(1);
  });

  it("ignores the ended event while a teardown is in progress", () => {
    const engine = new PreviewEngine();
    const onEnded = vi.fn();
    engine.onEnded = onEnded;
    engine.setMix(makeMix());
    engine.play();
    const source = lastSource();
    setStopping(engine, true);
    source.onended?.();
    expect(engine.transport).toBe("playing");
    expect(onEnded).not.toHaveBeenCalled();
    setStopping(engine, false);
  });

  it("does nothing when play is called without a mix", () => {
    const engine = new PreviewEngine();
    engine.play();
    expect(engine.transport).toBe("stopped");
    expect(env.ctx.createBufferSource).not.toHaveBeenCalled();
  });

  it("seeks within the buffer while stopped, clamping to its duration", () => {
    const engine = new PreviewEngine();
    engine.setMix(makeMix(1000, 1000));
    engine.seek(0.5);
    expect(engine.getPosition()).toBe(0.5);
    engine.seek(5);
    expect(engine.getPosition()).toBe(1);
  });

  it("seeks to the requested position when no mix is loaded", () => {
    const engine = new PreviewEngine();
    engine.seek(3);
    expect(engine.getPosition()).toBe(3);
  });

  it("restarts playback from the new position when seeking while playing", () => {
    const engine = new PreviewEngine();
    engine.setMix(makeMix(2000, 1000));
    env.ctx.currentTime = 1;
    engine.play();
    engine.seek(1.5);
    expect(engine.transport).toBe("playing");
    expect(lastSource().start).toHaveBeenLastCalledWith(0, 1.5);
  });
});
