import { describe, expect, it } from "vitest";
import { DEFAULT_BASE_PITCH, DEFAULT_SAMPLE_RATE, createEmptyProject, parseProject } from "./project";

describe("constants", () => {
  it("exposes default base pitch and sample rate", () => {
    expect(DEFAULT_BASE_PITCH).toBe(60);
    expect(DEFAULT_SAMPLE_RATE).toBe(48000);
  });
});

describe("parseProject", () => {
  it("applies defaults for a minimal project", () => {
    const project = parseProject({ version: 1, name: "Minimal" });
    expect(project).toMatchObject({
      version: 1,
      name: "Minimal",
      bpm: 140,
      ppq: 480,
      sampleRate: DEFAULT_SAMPLE_RATE,
      masterGain: 1,
      tempos: [],
      samples: [],
      tracks: [],
    });
  });

  it("fills sample defaults", () => {
    const project = parseProject({
      version: 1,
      name: "S",
      samples: [{ id: "s1", name: "kick" }],
    });
    expect(project.samples[0]).toEqual({
      id: "s1",
      name: "kick",
      fileName: "",
      basePitch: DEFAULT_BASE_PITCH,
      tuneCents: 0,
      gain: 1,
      durationSec: 0,
      interpolation: "hermite",
      loop: { enabled: false, startSec: 0, endSec: 0 },
      envelope: {
        delayMs: 0,
        attackMs: 4,
        attackCurve: 0,
        holdMs: 0,
        decayMs: 0,
        decayCurve: 0,
        sustain: 1,
        releaseMs: 90,
        releaseCurve: 0,
      },
      filter: {
        enabled: false,
        type: "lowpass",
        cutoffHz: 20000,
        q: 0.707,
        gainDb: 0,
        envAmount: 0,
        lfoHz: 5,
        lfoDepth: 0,
        lfoShape: "sine",
      },
      pitchMod: {
        glideSemitones: 0,
        glideMs: 0,
        glideCurve: 0,
        vibratoCents: 0,
        vibratoHz: 5,
        vibratoDelayMs: 0,
        vibratoFadeMs: 0,
        vibratoShape: "sine",
      },
    });
  });

  it("fills track and note defaults", () => {
    const project = parseProject({
      version: 1,
      name: "T",
      tracks: [{ id: "t1", name: "lead", notes: [{ pitch: 60, startSec: 0, durationSec: 1 }] }],
    });
    const track = project.tracks[0]!;
    expect(track).toMatchObject({
      color: "#7c5cff",
      muted: false,
      solo: false,
      gain: 1,
      pan: 0,
      defaultSampleId: null,
      noteSampleMap: {},
      dynamics: { volume: [], expression: [] },
    });
    expect(track.notes[0]!.velocity).toBe(100);
  });

  it("preserves explicitly provided values", () => {
    const project = parseProject({
      version: 1,
      name: "Full",
      bpm: 120,
      ppq: 96,
      sampleRate: 44100,
      masterGain: 0.8,
      tempos: [{ timeSec: 0, bpm: 120 }],
      samples: [
        {
          id: "s1",
          name: "sine",
          basePitch: 48,
          tuneCents: 10,
          gain: 2,
          durationSec: 1.5,
          loop: { enabled: true, startSec: 0.1, endSec: 0.9 },
          envelope: { attackMs: 5, releaseMs: 120 },
        },
      ],
      tracks: [
        {
          id: "t1",
          name: "lead",
          midiIndex: 2,
          color: "#36d399",
          muted: true,
          solo: true,
          gain: 0.5,
          pan: -0.5,
          defaultSampleId: "s1",
          noteSampleMap: { "60": "s1" },
          notes: [{ pitch: 60, startSec: 0, durationSec: 0.5, velocity: 80 }],
          dynamics: { volume: [{ t: 0, v: 1 }], expression: [{ t: 1, v: 0.2 }] },
        },
      ],
    });
    expect(project.bpm).toBe(120);
    expect(project.tempos).toEqual([{ timeSec: 0, bpm: 120 }]);
    expect(project.samples[0]!.loop.enabled).toBe(true);
    expect(project.tracks[0]!.noteSampleMap).toEqual({ "60": "s1" });
  });

  it("rejects a project without a version", () => {
    expect(() => parseProject({ name: "x" })).toThrow();
  });

  it("rejects an unsupported version", () => {
    expect(() => parseProject({ version: 2, name: "x" })).toThrow();
  });

  it("rejects an empty name", () => {
    expect(() => parseProject({ version: 1, name: "" })).toThrow();
  });

  it("rejects notes with non-positive duration", () => {
    expect(() =>
      parseProject({
        version: 1,
        name: "x",
        tracks: [{ id: "t1", name: "t", notes: [{ pitch: 60, startSec: 0, durationSec: 0 }] }],
      }),
    ).toThrow();
  });

  it("rejects pitches outside the MIDI range", () => {
    expect(() =>
      parseProject({
        version: 1,
        name: "x",
        tracks: [{ id: "t1", name: "t", notes: [{ pitch: 200, startSec: 0, durationSec: 1 }] }],
      }),
    ).toThrow();
  });
});

describe("extended synthesis schema", () => {
  it("defaults the track reverb send to zero", () => {
    const project = parseProject({
      version: 1,
      name: "T",
      tracks: [{ id: "t1", name: "lead" }],
    });
    expect(project.tracks[0]!.reverbSend).toBe(0);
  });

  it("defaults the project reverb bus to a disabled hall", () => {
    const project = parseProject({ version: 1, name: "R" });
    expect(project.reverb).toEqual({
      enabled: false,
      roomSize: 0.5,
      damping: 0.5,
      width: 1,
      wet: 0.25,
      dry: 1,
      preDelayMs: 0,
    });
  });

  it("preserves a fully specified envelope, filter and pitch modulation", () => {
    const project = parseProject({
      version: 1,
      name: "Synth",
      samples: [
        {
          id: "s1",
          name: "voice",
          interpolation: "linear",
          envelope: {
            delayMs: 5,
            attackMs: 10,
            attackCurve: 2,
            holdMs: 20,
            decayMs: 40,
            decayCurve: -1,
            sustain: 0.6,
            releaseMs: 200,
            releaseCurve: 3,
          },
          filter: { enabled: true, type: "bandpass", cutoffHz: 800, q: 4, gainDb: 6 },
          pitchMod: {
            glideSemitones: -12,
            glideMs: 80,
            glideCurve: 1,
            vibratoCents: 50,
            vibratoHz: 6,
            vibratoDelayMs: 100,
            vibratoFadeMs: 150,
            vibratoShape: "triangle",
          },
        },
      ],
    });
    const sample = project.samples[0]!;
    expect(sample.interpolation).toBe("linear");
    expect(sample.envelope.sustain).toBe(0.6);
    expect(sample.filter).toEqual({
      enabled: true,
      type: "bandpass",
      cutoffHz: 800,
      q: 4,
      gainDb: 6,
      envAmount: 0,
      lfoHz: 5,
      lfoDepth: 0,
      lfoShape: "sine",
    });
    expect(sample.pitchMod.vibratoShape).toBe("triangle");
  });

  it("rejects a sustain level above one", () => {
    expect(() =>
      parseProject({
        version: 1,
        name: "x",
        samples: [{ id: "s1", name: "s", envelope: { sustain: 1.5 } }],
      }),
    ).toThrow();
  });

  it("rejects an unknown filter type", () => {
    expect(() =>
      parseProject({
        version: 1,
        name: "x",
        samples: [{ id: "s1", name: "s", filter: { type: "comb" } }],
      }),
    ).toThrow();
  });

  it("rejects a reverb send outside the unit range", () => {
    expect(() =>
      parseProject({
        version: 1,
        name: "x",
        tracks: [{ id: "t1", name: "t", reverbSend: 2 }],
      }),
    ).toThrow();
  });
});

describe("track polyphony schema", () => {
  it("defaults to unlimited voices with newest priority and no stop grouping", () => {
    const project = parseProject({ version: 1, name: "P", tracks: [{ id: "t1", name: "t" }] });
    expect(project.tracks[0]!.polyphony).toEqual({ maxVoices: 0, priority: "newest", stopMode: "none" });
  });

  it("preserves an explicitly configured polyphony block", () => {
    const project = parseProject({
      version: 1,
      name: "P",
      tracks: [{ id: "t1", name: "t", polyphony: { maxVoices: 4, priority: "oldest", stopMode: "pitch" } }],
    });
    expect(project.tracks[0]!.polyphony).toEqual({ maxVoices: 4, priority: "oldest", stopMode: "pitch" });
  });

  it("rejects an unknown playback priority", () => {
    expect(() =>
      parseProject({ version: 1, name: "x", tracks: [{ id: "t1", name: "t", polyphony: { priority: "loudest" } }] }),
    ).toThrow();
  });

  it("rejects an unknown stop mode", () => {
    expect(() =>
      parseProject({ version: 1, name: "x", tracks: [{ id: "t1", name: "t", polyphony: { stopMode: "channel" } }] }),
    ).toThrow();
  });

  it("rejects a negative voice cap", () => {
    expect(() =>
      parseProject({ version: 1, name: "x", tracks: [{ id: "t1", name: "t", polyphony: { maxVoices: -1 } }] }),
    ).toThrow();
  });
});

describe("output settings schema", () => {
  it("defaults to an enabled soft limiter and a short trailing tail", () => {
    const project = parseProject({ version: 1, name: "O" });
    expect(project.output).toEqual({
      tailSec: 0.25,
      limiter: { enabled: true, threshold: 0.8 },
    });
  });

  it("preserves an explicitly configured output block", () => {
    const project = parseProject({
      version: 1,
      name: "O",
      output: { tailSec: 1.5, limiter: { enabled: false, threshold: 0.5 } },
    });
    expect(project.output).toEqual({
      tailSec: 1.5,
      limiter: { enabled: false, threshold: 0.5 },
    });
  });

  it("rejects a limiter threshold below the floor", () => {
    expect(() => parseProject({ version: 1, name: "x", output: { limiter: { threshold: 0.05 } } })).toThrow();
  });

  it("rejects a limiter threshold above unity", () => {
    expect(() => parseProject({ version: 1, name: "x", output: { limiter: { threshold: 1.5 } } })).toThrow();
  });

  it("rejects a negative trailing tail", () => {
    expect(() => parseProject({ version: 1, name: "x", output: { tailSec: -1 } })).toThrow();
  });

  it("rejects a trailing tail beyond the maximum", () => {
    expect(() => parseProject({ version: 1, name: "x", output: { tailSec: 11 } })).toThrow();
  });
});

describe("createEmptyProject", () => {
  it("creates a default-named project", () => {
    const project = createEmptyProject();
    expect(project.name).toBe("Untitled 音MAD");
    expect(project.version).toBe(1);
    expect(project.tracks).toEqual([]);
    expect(project.samples).toEqual([]);
  });

  it("accepts a custom name", () => {
    expect(createEmptyProject("My Song").name).toBe("My Song");
  });
});
