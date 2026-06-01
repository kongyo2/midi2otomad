import { describe, expect, it } from "vitest";
import { bankFromRecord, mixProject, velocityToGain, type PcmAudio } from "./mixer";
import { parseProject, type Project } from "../schemas/project";

function constSource(value: number, frames: number, sampleRate = 1000): PcmAudio {
  return { sampleRate, channels: [new Float32Array(frames).fill(value)], frames };
}

function rampSource(frames: number, sampleRate = 1000): PcmAudio {
  const ch = new Float32Array(frames);
  for (let i = 0; i < frames; i += 1) {
    ch[i] = (i % 100) / 100;
  }
  return { sampleRate, channels: [ch], frames };
}

interface ProjectOpts {
  sampleRate?: number;
  masterGain?: number;
  samples?: unknown[];
  tracks?: unknown[];
}

function makeProject(opts: ProjectOpts): Project {
  return parseProject({
    version: 1,
    name: "test",
    sampleRate: opts.sampleRate ?? 1000,
    masterGain: opts.masterGain ?? 1,
    samples: opts.samples ?? [],
    tracks: opts.tracks ?? [],
  });
}

function sampleRaw(over: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    id: "s1",
    name: "s",
    basePitch: 60,
    gain: 1,
    durationSec: 1,
    loop: { enabled: false, startSec: 0, endSec: 0 },
    envelope: { attackMs: 0, releaseMs: 0 },
    ...over,
  };
}

function trackRaw(over: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    id: "t1",
    name: "t",
    defaultSampleId: "s1",
    notes: [{ pitch: 60, startSec: 0, durationSec: 0.5, velocity: 127 }],
    dynamics: { volume: [], expression: [] },
    ...over,
  };
}

function allFinite(arr: Float32Array): boolean {
  for (let i = 0; i < arr.length; i += 1) {
    if (!Number.isFinite(arr[i])) {
      return false;
    }
  }
  return true;
}

describe("velocityToGain", () => {
  it("maps full velocity to unity gain", () => {
    expect(velocityToGain(127)).toBeCloseTo(1, 9);
  });

  it("maps zero velocity to silence", () => {
    expect(velocityToGain(0)).toBe(0);
  });

  it("clamps out-of-range velocities", () => {
    expect(velocityToGain(-50)).toBe(0);
    expect(velocityToGain(999)).toBeCloseTo(1, 9);
  });

  it("applies a curve below full velocity", () => {
    expect(velocityToGain(64)).toBeCloseTo(Math.pow(64 / 127, 1.35), 9);
  });
});

describe("bankFromRecord", () => {
  it("returns stored audio and undefined for misses", () => {
    const src = constSource(1, 4);
    const bank = bankFromRecord({ s1: src });
    expect(bank.get("s1")).toBe(src);
    expect(bank.get("missing")).toBeUndefined();
  });
});

describe("mixProject basics", () => {
  it("renders an assigned note into a stereo buffer", () => {
    const project = makeProject({ samples: [sampleRaw()], tracks: [trackRaw()] });
    const mix = mixProject(project, bankFromRecord({ s1: constSource(1, 1000) }));
    expect(mix.sampleRate).toBe(1000);
    expect(mix.peak).toBeGreaterThan(0);
    expect(mix.durationSec).toBeCloseTo(mix.frames / 1000, 9);
    expect(allFinite(mix.left)).toBe(true);
    expect(allFinite(mix.right)).toBe(true);
  });

  it("returns a silent buffer covering just the tail for an empty project", () => {
    const mix = mixProject(makeProject({}), bankFromRecord({}));
    expect(mix.frames).toBe(251);
    expect(mix.peak).toBe(0);
    expect(allFinite(mix.left)).toBe(true);
  });

  it("honors an explicit tailSec option", () => {
    const project = makeProject({ samples: [sampleRaw()], tracks: [trackRaw()] });
    const withTail = mixProject(project, bankFromRecord({ s1: constSource(1, 1000) }), { tailSec: 1 });
    const noTail = mixProject(project, bankFromRecord({ s1: constSource(1, 1000) }), { tailSec: 0 });
    expect(withTail.frames).toBeGreaterThan(noTail.frames);
  });
});

describe("mixProject voice selection", () => {
  it("silences muted tracks", () => {
    const project = makeProject({ samples: [sampleRaw()], tracks: [trackRaw({ muted: true })] });
    const mix = mixProject(project, bankFromRecord({ s1: constSource(1, 1000) }));
    expect(mix.peak).toBe(0);
  });

  it("plays only soloed tracks", () => {
    const project = makeProject({
      samples: [sampleRaw()],
      tracks: [trackRaw({ id: "solo", solo: true, pan: -1 }), trackRaw({ id: "muted-by-solo", solo: false, pan: 1 })],
    });
    const mix = mixProject(project, bankFromRecord({ s1: constSource(1, 1000) }), { limiter: false });
    expect(Math.abs(mix.left[50]!)).toBeGreaterThan(0);
    expect(mix.right[50]).toBe(0);
  });

  it("skips notes with no assigned sample", () => {
    const project = makeProject({ samples: [sampleRaw()], tracks: [trackRaw({ defaultSampleId: null })] });
    const mix = mixProject(project, bankFromRecord({ s1: constSource(1, 1000) }));
    expect(mix.peak).toBe(0);
  });

  it("skips notes whose sample is absent from the project", () => {
    const project = makeProject({ samples: [sampleRaw()], tracks: [trackRaw({ defaultSampleId: "ghost" })] });
    const mix = mixProject(project, bankFromRecord({ s1: constSource(1, 1000) }));
    expect(mix.peak).toBe(0);
  });

  it("skips notes with no decoded audio in the bank", () => {
    const project = makeProject({ samples: [sampleRaw()], tracks: [trackRaw()] });
    const mix = mixProject(project, bankFromRecord({}));
    expect(mix.peak).toBe(0);
  });

  it("skips sources shorter than two frames", () => {
    const project = makeProject({ samples: [sampleRaw()], tracks: [trackRaw()] });
    const mix = mixProject(project, bankFromRecord({ s1: constSource(1, 1) }));
    expect(mix.peak).toBe(0);
  });

  it("uses the per-note sample map override", () => {
    const project = makeProject({
      samples: [sampleRaw({ id: "a" }), sampleRaw({ id: "b" })],
      tracks: [
        trackRaw({
          defaultSampleId: "a",
          noteSampleMap: { "60": "b" },
          notes: [{ pitch: 60, startSec: 0, durationSec: 0.5, velocity: 127 }],
        }),
      ],
    });
    const mix = mixProject(project, bankFromRecord({ b: constSource(1, 1000) }));
    expect(mix.peak).toBeGreaterThan(0);
  });
});

describe("mixProject track dynamics", () => {
  function dynamicsGain(t: number, frame: number): number {
    const project = makeProject({
      samples: [sampleRaw()],
      tracks: [
        trackRaw({
          notes: [{ pitch: 60, startSec: 0, durationSec: 0.5, velocity: 127 }],
          dynamics: {
            volume: [
              { t: 0.1, v: 0.4 },
              { t: 0.3, v: 0.9 },
            ],
            expression: [],
          },
        }),
      ],
    });
    const mix = mixProject(project, bankFromRecord({ s1: constSource(1, 1000) }), { limiter: false });
    expect(t).toBeCloseTo(frame / 1000, 9);
    return mix.left[frame]!;
  }

  it("holds full volume before the first automation point", () => {
    expect(dynamicsGain(0.05, 50)).toBeCloseTo(1, 4);
  });

  it("interpolates linearly between automation points", () => {
    expect(dynamicsGain(0.2, 200)).toBeCloseTo(0.65, 4);
  });

  it("holds the last value after the final automation point", () => {
    expect(dynamicsGain(0.4, 400)).toBeCloseTo(0.9, 4);
  });

  it("multiplies volume and expression curves together", () => {
    const project = makeProject({
      samples: [sampleRaw()],
      tracks: [
        trackRaw({
          notes: [{ pitch: 60, startSec: 0, durationSec: 0.5, velocity: 127 }],
          dynamics: {
            volume: [{ t: 0, v: 0.5 }],
            expression: [{ t: 0, v: 0.5 }],
          },
        }),
      ],
    });
    const mix = mixProject(project, bankFromRecord({ s1: constSource(1, 1000) }), { limiter: false });
    expect(mix.left[100]!).toBeCloseTo(0.25, 4);
  });
});

describe("mixProject panning", () => {
  function panned(pan: number, mutate = false): { left: number; right: number } {
    const project = makeProject({ samples: [sampleRaw()], tracks: [trackRaw({ pan: mutate ? 0 : pan })] });
    if (mutate) {
      project.tracks[0]!.pan = pan;
    }
    const mix = mixProject(project, bankFromRecord({ s1: constSource(1, 1000) }), { limiter: false });
    return { left: mix.left[50]!, right: mix.right[50]! };
  }

  it("keeps both channels equal at center", () => {
    const { left, right } = panned(0);
    expect(left).toBeCloseTo(1, 5);
    expect(right).toBeCloseTo(1, 5);
  });

  it("attenuates the left channel when panned right", () => {
    const { left, right } = panned(0.5);
    expect(left).toBeCloseTo(0.5, 5);
    expect(right).toBeCloseTo(1, 5);
  });

  it("attenuates the right channel when panned left", () => {
    const { left, right } = panned(-0.5);
    expect(left).toBeCloseTo(1, 5);
    expect(right).toBeCloseTo(0.5, 5);
  });

  it("clamps panning beyond the right edge", () => {
    const { left, right } = panned(2, true);
    expect(left).toBeCloseTo(0, 5);
    expect(right).toBeCloseTo(1, 5);
  });

  it("clamps panning beyond the left edge", () => {
    const { left, right } = panned(-2, true);
    expect(left).toBeCloseTo(1, 5);
    expect(right).toBeCloseTo(0, 5);
  });
});

describe("mixProject envelope and limiter", () => {
  it("ramps in during the attack and skips silent onset frames", () => {
    const project = makeProject({
      samples: [sampleRaw({ envelope: { attackMs: 50, releaseMs: 5 } })],
      tracks: [trackRaw()],
    });
    const mix = mixProject(project, bankFromRecord({ s1: constSource(1, 1000) }), { limiter: false });
    expect(mix.left[0]).toBe(0);
    expect(Math.abs(mix.left[25]!)).toBeLessThan(Math.abs(mix.left[49]!));
  });

  it("soft-clips peaks above the threshold on both polarities", () => {
    const square = new Float32Array(1000);
    for (let i = 0; i < square.length; i += 1) {
      square[i] = i % 2 === 0 ? 1 : -1;
    }
    const project = makeProject({
      samples: [sampleRaw({ envelope: { attackMs: 20, releaseMs: 5 } })],
      tracks: [trackRaw()],
    });
    const mix = mixProject(project, bankFromRecord({ s1: { sampleRate: 1000, channels: [square], frames: 1000 } }));
    expect(mix.peak).toBeGreaterThan(0.8);
    let maxAbs = 0;
    for (let i = 0; i < mix.frames; i += 1) {
      maxAbs = Math.max(maxAbs, Math.abs(mix.left[i]!));
    }
    expect(maxAbs).toBeLessThan(1);
  });

  it("leaves the buffer untouched when the limiter is disabled", () => {
    const project = makeProject({ samples: [sampleRaw()], tracks: [trackRaw()] });
    const mix = mixProject(project, bankFromRecord({ s1: constSource(1, 1000) }), { limiter: false });
    expect(mix.left[100]).toBeCloseTo(1, 5);
  });
});

describe("mixProject source edge cases", () => {
  it("renders stereo sources using both channels", () => {
    const left = new Float32Array(1000).fill(0.5);
    const right = new Float32Array(1000).fill(-0.25);
    const project = makeProject({ samples: [sampleRaw()], tracks: [trackRaw({ pan: 0 })] });
    const mix = mixProject(
      project,
      bankFromRecord({ s1: { sampleRate: 1000, channels: [left, right], frames: 1000 } }),
      {
        limiter: false,
      },
    );
    expect(mix.left[100]!).toBeCloseTo(0.5, 5);
    expect(mix.right[100]!).toBeCloseTo(-0.25, 5);
  });

  it("ignores a source that has no channels", () => {
    const project = makeProject({ samples: [sampleRaw()], tracks: [trackRaw()] });
    const mix = mixProject(project, bankFromRecord({ s1: { sampleRate: 1000, channels: [], frames: 1000 } }));
    expect(mix.peak).toBe(0);
  });

  it("zeroes out NaN and infinite sample values", () => {
    const ch = new Float32Array(1000).fill(0.3);
    ch[1] = Number.NaN;
    ch[2] = Number.POSITIVE_INFINITY;
    ch[3] = Number.NEGATIVE_INFINITY;
    const project = makeProject({ samples: [sampleRaw()], tracks: [trackRaw()] });
    const mix = mixProject(project, bankFromRecord({ s1: { sampleRate: 1000, channels: [ch], frames: 1000 } }), {
      limiter: false,
    });
    expect(allFinite(mix.left)).toBe(true);
    expect(allFinite(mix.right)).toBe(true);
  });

  it("stops reading a non-looping source at its end", () => {
    const project = makeProject({ samples: [sampleRaw()], tracks: [trackRaw()] });
    const mix = mixProject(project, bankFromRecord({ s1: constSource(1, 100) }), { limiter: false });
    expect(Math.abs(mix.left[10]!)).toBeGreaterThan(0);
    expect(mix.left[400]).toBe(0);
  });
});

describe("mixProject looping", () => {
  it("wraps playback through an enabled loop region", () => {
    const project = makeProject({
      samples: [sampleRaw({ loop: { enabled: true, startSec: 0.1, endSec: 0.3 } })],
      tracks: [trackRaw()],
    });
    const mix = mixProject(project, bankFromRecord({ s1: rampSource(1000) }), { limiter: false });
    expect(mix.peak).toBeGreaterThan(0);
    expect(allFinite(mix.left)).toBe(true);
  });

  it("falls back to the full sample when the loop end is not past the start", () => {
    const project = makeProject({
      samples: [sampleRaw({ loop: { enabled: true, startSec: 0.1, endSec: 0.05 } })],
      tracks: [trackRaw()],
    });
    const mix = mixProject(project, bankFromRecord({ s1: rampSource(1000) }), { limiter: false });
    expect(mix.peak).toBeGreaterThan(0);
  });

  it("ignores a degenerate loop region shorter than two frames", () => {
    const project = makeProject({
      samples: [sampleRaw({ loop: { enabled: true, startSec: 0.9, endSec: 0.9001 } })],
      tracks: [trackRaw()],
    });
    const mix = mixProject(project, bankFromRecord({ s1: rampSource(1000) }), { limiter: false });
    expect(mix.peak).toBeGreaterThan(0);
  });
});

describe("mixProject buffer bounds", () => {
  it("skips frames scheduled before time zero", () => {
    const project = makeProject({ samples: [sampleRaw()], tracks: [trackRaw()] });
    project.tracks[0]!.notes[0]!.startSec = -0.05;
    const mix = mixProject(project, bankFromRecord({ s1: constSource(1, 1000) }), { limiter: false });
    expect(mix.peak).toBeGreaterThan(0);
    expect(allFinite(mix.left)).toBe(true);
  });

  it("stops writing once the note runs past the end of the buffer", () => {
    const project = makeProject({ samples: [sampleRaw()], tracks: [trackRaw()] });
    const mix = mixProject(project, bankFromRecord({ s1: constSource(1, 1000) }), { tailSec: -0.4 });
    expect(mix.frames).toBe(101);
    expect(allFinite(mix.left)).toBe(true);
  });

  it("never produces fewer than one frame", () => {
    const project = makeProject({ samples: [sampleRaw()], tracks: [trackRaw()] });
    const mix = mixProject(project, bankFromRecord({ s1: constSource(1, 1000) }), { tailSec: -10 });
    expect(mix.frames).toBeGreaterThanOrEqual(1);
  });
});
