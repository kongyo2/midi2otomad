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
      loop: { enabled: false, startSec: 0, endSec: 0 },
      envelope: { attackMs: 4, releaseMs: 90 },
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
