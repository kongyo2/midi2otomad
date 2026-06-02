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

function monoRampSource(frames: number, sampleRate = 1000): PcmAudio {
  const ch = new Float32Array(frames);
  for (let i = 0; i < frames; i += 1) {
    ch[i] = i / frames;
  }
  return { sampleRate, channels: [ch], frames };
}

function nyquistSource(frames: number, sampleRate = 1000): PcmAudio {
  const ch = new Float32Array(frames);
  for (let i = 0; i < frames; i += 1) {
    ch[i] = i % 2 === 0 ? 1 : -1;
  }
  return { sampleRate, channels: [ch], frames };
}

/** A bright quarter-rate square (250 Hz at 1 kHz) — high energy but below Nyquist, so a lowpass can shape it. */
function brightSource(frames: number, sampleRate = 1000): PcmAudio {
  const ch = new Float32Array(frames);
  for (let i = 0; i < frames; i += 1) {
    ch[i] = Math.floor(i / 2) % 2 === 0 ? 1 : -1;
  }
  return { sampleRate, channels: [ch], frames };
}

function tailEnergy(arr: Float32Array, start: number): number {
  let sum = 0;
  for (let i = start; i < arr.length; i += 1) {
    sum += arr[i]! * arr[i]!;
  }
  return sum;
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

describe("mixProject interpolation quality", () => {
  function curved(frames: number): PcmAudio {
    const ch = new Float32Array(frames);
    for (let i = 0; i < frames; i += 1) {
      ch[i] = Math.sin(i * 0.3);
    }
    return { sampleRate: 1000, channels: [ch], frames };
  }

  it("defaults to cubic hermite, diverging from linear on fractional reads", () => {
    const src = curved(200);
    const base = sampleRaw({ tuneCents: 100, envelope: { attackMs: 0, releaseMs: 0 } });
    const hermiteMix = mixProject(
      makeProject({ samples: [{ ...base, interpolation: "hermite" }], tracks: [trackRaw()] }),
      bankFromRecord({ s1: src }),
      { limiter: false },
    );
    const linearMix = mixProject(
      makeProject({ samples: [{ ...base, interpolation: "linear" }], tracks: [trackRaw()] }),
      bankFromRecord({ s1: src }),
      { limiter: false },
    );
    let diverges = false;
    for (let i = 5; i < 150; i += 1) {
      if (Math.abs(hermiteMix.left[i]! - linearMix.left[i]!) > 1e-7) {
        diverges = true;
        break;
      }
    }
    expect(diverges).toBe(true);
    expect(allFinite(hermiteMix.left)).toBe(true);
    expect(allFinite(linearMix.left)).toBe(true);
  });
});

describe("mixProject full envelope", () => {
  it("decays toward the sustain level while the note is held", () => {
    const project = makeProject({
      samples: [sampleRaw({ envelope: { attackMs: 0, decayMs: 100, sustain: 0.5, releaseMs: 0 } })],
      tracks: [trackRaw()],
    });
    const mix = mixProject(project, bankFromRecord({ s1: constSource(1, 1000) }), { limiter: false });
    expect(mix.left[200]!).toBeCloseTo(0.5, 3);
    expect(mix.left[50]!).toBeCloseTo(0.75, 3);
  });

  it("stays silent during the delay stage and opens afterward", () => {
    const project = makeProject({
      samples: [sampleRaw({ envelope: { delayMs: 50, attackMs: 0, releaseMs: 0 } })],
      tracks: [trackRaw()],
    });
    const mix = mixProject(project, bankFromRecord({ s1: constSource(1, 1000) }), { limiter: false });
    expect(mix.left[20]).toBe(0);
    expect(mix.left[80]!).toBeCloseTo(1, 3);
  });
});

describe("mixProject dynamic pitch", () => {
  it("reads further into a rising sample as the pitch glides up", () => {
    const src = monoRampSource(1000);
    const glided = mixProject(
      makeProject({
        samples: [
          sampleRaw({ envelope: { attackMs: 0, releaseMs: 0 }, pitchMod: { glideSemitones: 12, glideMs: 1000 } }),
        ],
        tracks: [trackRaw()],
      }),
      bankFromRecord({ s1: src }),
      { limiter: false },
    );
    const plain = mixProject(
      makeProject({
        samples: [sampleRaw({ envelope: { attackMs: 0, releaseMs: 0 } })],
        tracks: [trackRaw()],
      }),
      bankFromRecord({ s1: src }),
      { limiter: false },
    );
    expect(glided.left[100]!).toBeGreaterThan(plain.left[100]!);
  });

  it("wobbles playback under vibrato", () => {
    const src = monoRampSource(1000);
    const vibrato = mixProject(
      makeProject({
        samples: [
          sampleRaw({ envelope: { attackMs: 0, releaseMs: 0 }, pitchMod: { vibratoCents: 200, vibratoHz: 8 } }),
        ],
        tracks: [trackRaw()],
      }),
      bankFromRecord({ s1: src }),
      { limiter: false },
    );
    const plain = mixProject(
      makeProject({
        samples: [sampleRaw({ envelope: { attackMs: 0, releaseMs: 0 } })],
        tracks: [trackRaw()],
      }),
      bankFromRecord({ s1: src }),
      { limiter: false },
    );
    let wobbles = false;
    for (let i = 10; i < 400; i += 1) {
      if (Math.abs(vibrato.left[i]! - plain.left[i]!) > 1e-6) {
        wobbles = true;
        break;
      }
    }
    expect(wobbles).toBe(true);
  });
});

describe("mixProject per-sample filter", () => {
  it("tames a bright source with a lowpass filter", () => {
    const src = nyquistSource(1000);
    const filtered = mixProject(
      makeProject({
        samples: [
          sampleRaw({
            envelope: { attackMs: 0, releaseMs: 0 },
            filter: { enabled: true, type: "lowpass", cutoffHz: 50 },
          }),
        ],
        tracks: [trackRaw()],
      }),
      bankFromRecord({ s1: src }),
      { limiter: false },
    );
    const open = mixProject(
      makeProject({
        samples: [sampleRaw({ envelope: { attackMs: 0, releaseMs: 0 } })],
        tracks: [trackRaw()],
      }),
      bankFromRecord({ s1: src }),
      { limiter: false },
    );
    expect(filtered.peak).toBeLessThan(open.peak);
    expect(allFinite(filtered.left)).toBe(true);
  });

  it("opens the cutoff as the envelope amount rises", () => {
    const src = brightSource(1000);
    const swept = mixProject(
      makeProject({
        samples: [
          sampleRaw({
            envelope: { attackMs: 0, releaseMs: 0 },
            filter: { enabled: true, type: "lowpass", cutoffHz: 80, envAmount: 4 },
          }),
        ],
        tracks: [trackRaw()],
      }),
      bankFromRecord({ s1: src }),
      { limiter: false },
    );
    const closed = mixProject(
      makeProject({
        samples: [
          sampleRaw({
            envelope: { attackMs: 0, releaseMs: 0 },
            filter: { enabled: true, type: "lowpass", cutoffHz: 80, envAmount: 0 },
          }),
        ],
        tracks: [trackRaw()],
      }),
      bankFromRecord({ s1: src }),
      { limiter: false },
    );
    expect(swept.peak).toBeGreaterThan(closed.peak);
    expect(allFinite(swept.left)).toBe(true);
  });

  it("wobbles the cutoff with the filter LFO, brightening at the crest", () => {
    const src = brightSource(2000);
    const mix = mixProject(
      makeProject({
        samples: [
          sampleRaw({
            durationSec: 2,
            envelope: { attackMs: 0, releaseMs: 0 },
            filter: { enabled: true, type: "lowpass", cutoffHz: 200, lfoDepth: 4, lfoHz: 2, lfoShape: "sine" },
          }),
        ],
        tracks: [trackRaw({ notes: [{ pitch: 60, startSec: 0, durationSec: 1.5, velocity: 127 }] })],
      }),
      bankFromRecord({ s1: src }),
      { limiter: false },
    );
    // LFO at 2 Hz, second cycle (past the filter's startup transient): the cutoff
    // crests near t=0.625s and troughs near t=0.875s, so the bright source rings
    // through far more strongly at the crest than at the trough.
    const energy = (center: number): number => {
      let sum = 0;
      for (let i = center - 20; i < center + 20; i += 1) {
        sum += mix.left[i]! * mix.left[i]!;
      }
      return sum;
    };
    expect(energy(625)).toBeGreaterThan(energy(875) * 2);
  });

  it("keeps an above-Nyquist cutoff stable at low render rates", () => {
    const src = brightSource(4000, 8000);
    const project = makeProject({
      sampleRate: 8000,
      samples: [
        sampleRaw({
          envelope: { attackMs: 0, releaseMs: 0 },
          filter: { enabled: true, type: "lowpass", cutoffHz: 6000 },
        }),
      ],
      tracks: [trackRaw()],
    });
    const mix = mixProject(project, bankFromRecord({ s1: src }), { limiter: false });
    expect(allFinite(mix.left)).toBe(true);
    expect(mix.peak).toBeLessThan(8);
  });
});

describe("mixProject reverb send", () => {
  function reverbProject(reverb: Record<string, unknown> | undefined, reverbSend: number): Project {
    return parseProject({
      version: 1,
      name: "r",
      sampleRate: 1000,
      ...(reverb !== undefined ? { reverb } : {}),
      samples: [sampleRaw({ envelope: { attackMs: 0, releaseMs: 0 } })],
      tracks: [trackRaw({ reverbSend, notes: [{ pitch: 60, startSec: 0, durationSec: 0.05, velocity: 127 }] })],
    });
  }

  it("adds a wet tail beyond the dry note when enabled and sent", () => {
    const project = reverbProject({ enabled: true, roomSize: 0.8, wet: 1, damping: 0.2 }, 1);
    const mix = mixProject(project, bankFromRecord({ s1: constSource(1, 1000) }), { limiter: false });
    expect(tailEnergy(mix.left, 600)).toBeGreaterThan(0);
  });

  it("stays dry when reverb is enabled but the track send is zero", () => {
    const project = reverbProject({ enabled: true, roomSize: 0.8, wet: 1 }, 0);
    const mix = mixProject(project, bankFromRecord({ s1: constSource(1, 1000) }), { limiter: false });
    expect(tailEnergy(mix.left, 600)).toBe(0);
  });

  it("stays dry when the reverb bus is disabled", () => {
    const project = reverbProject(undefined, 1);
    const mix = mixProject(project, bankFromRecord({ s1: constSource(1, 1000) }), { limiter: false });
    expect(tailEnergy(mix.left, 600)).toBe(0);
  });

  it("extends the render tail to fit a long reverb decay", () => {
    const project = reverbProject({ enabled: true, roomSize: 1, wet: 1 }, 1);
    const mix = mixProject(project, bankFromRecord({ s1: constSource(1, 1000) }), { limiter: false });
    expect(mix.durationSec).toBeGreaterThan(10);
  });

  it("does not reserve a long tail when no track sends to the reverb", () => {
    const project = reverbProject({ enabled: true, roomSize: 1, wet: 1 }, 0);
    const mix = mixProject(project, bankFromRecord({ s1: constSource(1, 1000) }), { limiter: false });
    expect(mix.durationSec).toBeLessThan(2);
  });

  it("stays dry when the reverb wet level is zero", () => {
    const project = reverbProject({ enabled: true, roomSize: 1, wet: 0 }, 1);
    const mix = mixProject(project, bankFromRecord({ s1: constSource(1, 1000) }), { limiter: false });
    expect(mix.durationSec).toBeLessThan(2);
    expect(tailEnergy(mix.left, 600)).toBe(0);
  });

  it("ignores a muted track's send when reserving the reverb tail", () => {
    const project = parseProject({
      version: 1,
      name: "r",
      sampleRate: 1000,
      reverb: { enabled: true, roomSize: 1, wet: 1 },
      samples: [sampleRaw({ envelope: { attackMs: 0, releaseMs: 0 } })],
      tracks: [
        trackRaw({
          muted: true,
          reverbSend: 1,
          notes: [{ pitch: 60, startSec: 0, durationSec: 0.05, velocity: 127 }],
        }),
      ],
    });
    const mix = mixProject(project, bankFromRecord({ s1: constSource(1, 1000) }), { limiter: false });
    expect(mix.durationSec).toBeLessThan(2);
  });

  it("does not reserve a long tail when the sending track has no decoded audio", () => {
    const project = reverbProject({ enabled: true, roomSize: 1, wet: 1 }, 1);
    const mix = mixProject(project, bankFromRecord({}), { limiter: false });
    expect(mix.durationSec).toBeLessThan(2);
    expect(tailEnergy(mix.left, 600)).toBe(0);
  });

  it("does not reserve a long tail when the sending track has no sample assigned", () => {
    const project = parseProject({
      version: 1,
      name: "r",
      sampleRate: 1000,
      reverb: { enabled: true, roomSize: 1, wet: 1 },
      samples: [sampleRaw({ envelope: { attackMs: 0, releaseMs: 0 } })],
      tracks: [
        trackRaw({
          defaultSampleId: null,
          reverbSend: 1,
          notes: [{ pitch: 60, startSec: 0, durationSec: 0.05, velocity: 127 }],
        }),
      ],
    });
    const mix = mixProject(project, bankFromRecord({ s1: constSource(1, 1000) }), { limiter: false });
    expect(mix.durationSec).toBeLessThan(2);
  });
});

describe("mixProject polyphony", () => {
  const dry = sampleRaw({ envelope: { attackMs: 0, releaseMs: 0 } });

  function polyMix(notes: unknown[], polyphony: Record<string, unknown>): Float32Array {
    const project = makeProject({ samples: [dry], tracks: [trackRaw({ notes, polyphony })] });
    return mixProject(project, bankFromRecord({ s1: constSource(1, 1000) }), { limiter: false }).left;
  }

  it("lets a held note keep ringing when stop grouping is off", () => {
    const notes = [
      { pitch: 60, startSec: 0, durationSec: 1, velocity: 127 },
      { pitch: 60, startSec: 0.5, durationSec: 0.05, velocity: 127 },
    ];
    expect(Math.abs(polyMix(notes, { stopMode: "none" })[800]!)).toBeGreaterThan(0);
  });

  it("chokes the earlier same-pitch note under pitch stop mode", () => {
    const notes = [
      { pitch: 60, startSec: 0, durationSec: 1, velocity: 127 },
      { pitch: 60, startSec: 0.5, durationSec: 0.05, velocity: 127 },
    ];
    expect(polyMix(notes, { stopMode: "pitch" })[800]).toBe(0);
  });

  it("sums every overlapping voice when the cap is unlimited", () => {
    const notes = [
      { pitch: 60, startSec: 0, durationSec: 1, velocity: 127 },
      { pitch: 64, startSec: 0.2, durationSec: 1, velocity: 127 },
      { pitch: 67, startSec: 0.4, durationSec: 1, velocity: 127 },
    ];
    expect(polyMix(notes, { maxVoices: 0 })[600]!).toBeCloseTo(3, 5);
  });

  it("steals the oldest voice once a track exceeds its cap", () => {
    const notes = [
      { pitch: 60, startSec: 0, durationSec: 1, velocity: 127 },
      { pitch: 64, startSec: 0.2, durationSec: 1, velocity: 127 },
      { pitch: 67, startSec: 0.4, durationSec: 1, velocity: 127 },
    ];
    expect(polyMix(notes, { maxVoices: 2 })[600]!).toBeCloseTo(2, 5);
  });

  it("fades a stolen voice quickly instead of dragging its full release tail", () => {
    const project = makeProject({
      samples: [
        sampleRaw({ id: "long", envelope: { attackMs: 0, releaseMs: 500 } }),
        sampleRaw({ id: "short", envelope: { attackMs: 0, releaseMs: 0 } }),
      ],
      tracks: [
        trackRaw({
          defaultSampleId: "long",
          noteSampleMap: { "64": "short" },
          notes: [
            { pitch: 60, startSec: 0, durationSec: 2, velocity: 127 },
            { pitch: 64, startSec: 0.5, durationSec: 0.1, velocity: 127 },
          ],
          polyphony: { maxVoices: 1, priority: "newest", stopMode: "none" },
        }),
      ],
    });
    const mix = mixProject(project, bankFromRecord({ long: constSource(1, 3000), short: constSource(1, 3000) }), {
      limiter: false,
    });
    expect(mix.left[800]).toBe(0);
  });

  it("frees a one-shot voice when its sample ends so a later capped note still plays", () => {
    const project = makeProject({
      samples: [sampleRaw({ id: "hit", envelope: { attackMs: 0, releaseMs: 0 } })],
      tracks: [
        trackRaw({
          defaultSampleId: "hit",
          notes: [
            { pitch: 60, startSec: 0, durationSec: 2, velocity: 127 },
            { pitch: 60, startSec: 1, durationSec: 0.2, velocity: 127 },
          ],
          polyphony: { maxVoices: 1, priority: "oldest", stopMode: "none" },
        }),
      ],
    });
    const mix = mixProject(project, bankFromRecord({ hit: constSource(1, 200) }), { limiter: false });
    expect(mix.left[1100]!).toBeGreaterThan(0);
  });

  it("holds a looping voice for the whole note, so the later capped note is dropped", () => {
    const project = makeProject({
      samples: [
        sampleRaw({
          id: "pad",
          envelope: { attackMs: 0, releaseMs: 0 },
          loop: { enabled: true, startSec: 0, endSec: 0.2 },
        }),
        sampleRaw({ id: "beep", envelope: { attackMs: 0, releaseMs: 0 } }),
      ],
      tracks: [
        trackRaw({
          defaultSampleId: "pad",
          noteSampleMap: { "72": "beep" },
          notes: [
            { pitch: 60, startSec: 0, durationSec: 2, velocity: 127 },
            { pitch: 72, startSec: 1, durationSec: 0.5, velocity: 127 },
          ],
          polyphony: { maxVoices: 1, priority: "oldest", stopMode: "none" },
        }),
      ],
    });
    const mix = mixProject(project, bankFromRecord({ pad: constSource(1, 200), beep: constSource(0.5, 2000) }), {
      limiter: false,
    });
    expect(mix.left[1100]!).toBeCloseTo(1, 5);
  });

  it("counts a release tail toward the cap, choking it when a new note arrives", () => {
    const project = makeProject({
      samples: [
        sampleRaw({ id: "rel", envelope: { attackMs: 0, releaseMs: 500 } }),
        sampleRaw({ id: "dry", envelope: { attackMs: 0, releaseMs: 0 } }),
      ],
      tracks: [
        trackRaw({
          defaultSampleId: "rel",
          noteSampleMap: { "64": "dry" },
          notes: [
            { pitch: 60, startSec: 0, durationSec: 0.1, velocity: 127 },
            { pitch: 64, startSec: 0.3, durationSec: 0.1, velocity: 127 },
          ],
          polyphony: { maxVoices: 1, priority: "newest", stopMode: "none" },
        }),
      ],
    });
    const mix = mixProject(project, bankFromRecord({ rel: constSource(1, 3000), dry: constSource(1, 3000) }), {
      limiter: false,
    });
    expect(mix.left[500]).toBe(0);
  });

  it("sizes the render from surviving voices, not dropped ones", () => {
    const project = makeProject({
      samples: [sampleRaw()],
      tracks: [
        trackRaw({
          notes: [
            { pitch: 60, startSec: 0, durationSec: 5, velocity: 127 },
            { pitch: 64, startSec: 0, durationSec: 0.1, velocity: 127 },
          ],
          polyphony: { maxVoices: 1, priority: "newest", stopMode: "none" },
        }),
      ],
    });
    const mix = mixProject(project, bankFromRecord({ s1: constSource(1, 6000) }), { limiter: false });
    expect(mix.frames).toBeLessThan(1000);
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
