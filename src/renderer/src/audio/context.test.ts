import { afterEach, describe, expect, it, vi } from "vitest";

afterEach(() => {
  vi.unstubAllGlobals();
  vi.resetModules();
});

describe("getAudioContext", () => {
  it("creates a context at the default sample rate and caches it", async () => {
    const options: unknown[] = [];
    class FakeAudioContext {
      state = "suspended";
      constructor(opts?: unknown) {
        options.push(opts);
      }
      resume = vi.fn(async () => {
        this.state = "running";
      });
    }
    vi.stubGlobal("AudioContext", FakeAudioContext);
    vi.resetModules();
    const mod = await import("./context");

    const ctx = mod.getAudioContext();
    expect(ctx).toBeInstanceOf(FakeAudioContext);
    expect(options[0]).toEqual({ sampleRate: 48000 });
    expect(mod.getAudioContext()).toBe(ctx);
    expect(options).toHaveLength(1);
  });

  it("falls back to a default context when the sample rate is rejected", async () => {
    class PickyAudioContext {
      state = "running";
      constructor(opts?: unknown) {
        if (opts !== undefined) {
          throw new Error("unsupported sample rate");
        }
      }
    }
    vi.stubGlobal("AudioContext", PickyAudioContext);
    vi.resetModules();
    const mod = await import("./context");

    expect(mod.getAudioContext()).toBeInstanceOf(PickyAudioContext);
  });
});

describe("resumeAudioContext", () => {
  it("resumes a suspended context and leaves a running one alone", async () => {
    const resume = vi.fn(async function (this: { state: string }) {
      this.state = "running";
    });
    class FakeAudioContext {
      state = "suspended";
      resume = resume;
    }
    vi.stubGlobal("AudioContext", FakeAudioContext);
    vi.resetModules();
    const mod = await import("./context");

    await mod.resumeAudioContext();
    expect(resume).toHaveBeenCalledTimes(1);

    await mod.resumeAudioContext();
    expect(resume).toHaveBeenCalledTimes(1);
  });
});
