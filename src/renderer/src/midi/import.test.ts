import { Midi } from "@tonejs/midi";
import { describe, expect, it } from "vitest";
import { midiToProject } from "./import";
import { parseProject, type Project } from "../../../shared/schemas/project";

function bytesOf(midi: Midi): Uint8Array {
  return midi.toArray();
}

describe("midiToProject", () => {
  it("imports tracks, notes and an expression curve", () => {
    const midi = new Midi();
    const track = midi.addTrack();
    track.name = "Melody";
    track.addNote({ midi: 60, time: 0, duration: 0.5, velocity: 0.8 });
    track.addNote({ midi: 67, time: 0.5, duration: 0.5, velocity: 0.6 });
    track.addCC({ number: 11, value: 0.9, time: 0 });
    track.addCC({ number: 11, value: 0.2, time: 0.4 });

    const result = midiToProject(bytesOf(midi), "demo.mid");
    expect(result.trackCount).toBe(1);
    expect(result.noteCount).toBe(2);
    expect(result.project.name).toBe("demo");

    const imported = result.project.tracks[0]!;
    expect(imported.name).toBe("Melody");
    expect(imported.color).toBe("#7c5cff");
    expect(imported.notes[0]!.pitch).toBe(60);
    expect(imported.notes[1]!.startSec).toBeCloseTo(0.5, 2);
    expect(imported.dynamics.expression.length).toBeGreaterThanOrEqual(2);
    expect(imported.dynamics.volume).toEqual([]);
    expect(imported.defaultSampleId).toBeNull();
  });

  it("drops empty tracks, names anonymous tracks and reads CC7 volume", () => {
    const midi = new Midi();
    midi.addTrack(); // no notes -> filtered out
    const named = midi.addTrack();
    named.name = "   ";
    named.addNote({ midi: 48, time: 0, duration: 1, velocity: 0.5 });
    named.addCC({ number: 7, value: 0.7, time: 0 });

    const result = midiToProject(bytesOf(midi), "song.midi");
    expect(result.trackCount).toBe(1);
    const track = result.project.tracks[0]!;
    expect(track.name).toBe("Track 1");
    expect(track.dynamics.volume.length).toBeGreaterThanOrEqual(1);
    expect(track.dynamics.expression).toEqual([]);
  });

  it("clamps note values into the supported ranges", () => {
    const midi = new Midi();
    const track = midi.addTrack();
    track.addNote({ midi: 60, time: 0, duration: 0.005, velocity: 0.8 });

    const result = midiToProject(bytesOf(midi), "x.mid");
    const note = result.project.tracks[0]!.notes[0]!;
    expect(note.pitch).toBe(60);
    expect(note.durationSec).toBeGreaterThanOrEqual(0.02);
    expect(note.velocity).toBeGreaterThanOrEqual(1);
    expect(note.velocity).toBeLessThanOrEqual(127);
  });

  it("reads tempo from the MIDI header when present", () => {
    const midi = new Midi();
    midi.header.tempos.push({ ticks: 0, bpm: 90 });
    midi.header.update();
    const track = midi.addTrack();
    track.addNote({ midi: 60, time: 0, duration: 0.5, velocity: 0.8 });

    const result = midiToProject(bytesOf(midi), "tempo.mid");
    expect(result.project.bpm).toBeCloseTo(90, 2);
    expect(result.project.tempos[0]!.bpm).toBeCloseTo(90, 2);
    expect(result.project.tempos[0]!.timeSec).toBeCloseTo(0, 6);
  });

  it("preserves a previous project's material and settings", () => {
    const previous: Project = parseProject({
      version: 1,
      name: "prev",
      bpm: 100,
      sampleRate: 44100,
      masterGain: 0.5,
      reverb: { enabled: true, roomSize: 0.9, wet: 0.4 },
      samples: [{ id: "kept", name: "snare" }],
      tracks: [],
    });

    const midi = new Midi();
    const track = midi.addTrack();
    track.addNote({ midi: 64, time: 0, duration: 0.25, velocity: 0.9 });

    const result = midiToProject(bytesOf(midi), "reimport.mid", previous);
    expect(result.project.sampleRate).toBe(44100);
    expect(result.project.masterGain).toBe(0.5);
    expect(result.project.reverb).toEqual(previous.reverb);
    expect(result.project.samples).toEqual(previous.samples);
    expect(result.project.tracks[0]!.defaultSampleId).toBe("kept");
    expect(result.project.bpm).toBe(100);
  });

  it("preserves per-track reverb sends across a re-import", () => {
    const previous: Project = parseProject({
      version: 1,
      name: "prev",
      reverb: { enabled: true, wet: 0.5 },
      tracks: [
        { id: "old", name: "lead", midiIndex: 0, reverbSend: 0.6, notes: [] },
        { id: "manual", name: "no-midi", reverbSend: 0.9, notes: [] },
      ],
    });

    const midi = new Midi();
    const track = midi.addTrack();
    track.addNote({ midi: 60, time: 0, duration: 0.5, velocity: 0.8 });

    const result = midiToProject(bytesOf(midi), "again.mid", previous);
    expect(result.project.tracks[0]!.reverbSend).toBe(0.6);
  });

  it("preserves per-track sends by MIDI position when a track gains notes on re-import", () => {
    const first = new Midi();
    first.addTrack().name = "drums";
    const bass1 = first.addTrack();
    bass1.name = "bass";
    bass1.addNote({ midi: 36, time: 0, duration: 0.5, velocity: 0.8 });

    const p1 = midiToProject(bytesOf(first), "song.mid").project;
    expect(p1.tracks).toHaveLength(1);
    p1.tracks[0]!.reverbSend = 0.7;

    const second = new Midi();
    const drums2 = second.addTrack();
    drums2.name = "drums";
    drums2.addNote({ midi: 38, time: 0, duration: 0.25, velocity: 0.9 });
    const bass2 = second.addTrack();
    bass2.name = "bass";
    bass2.addNote({ midi: 36, time: 0, duration: 0.5, velocity: 0.8 });

    const p2 = midiToProject(bytesOf(second), "song.mid", p1).project;
    const bass = p2.tracks.find((t) => t.name === "bass")!;
    const drums = p2.tracks.find((t) => t.name === "drums")!;
    expect(bass.reverbSend).toBe(0.7);
    expect(drums.reverbSend).toBe(0);
  });

  it("falls back to the default tempo without a header tempo or previous project", () => {
    const midi = new Midi();
    const track = midi.addTrack();
    track.addNote({ midi: 60, time: 0, duration: 0.5, velocity: 0.8 });

    const result = midiToProject(bytesOf(midi), "plain.mid");
    expect(result.project.bpm).toBe(140);
    expect(result.project.sampleRate).toBe(48000);
    expect(result.project.masterGain).toBe(1);
  });

  it("uses a fallback project name when the file name has no stem", () => {
    const midi = new Midi();
    const track = midi.addTrack();
    track.addNote({ midi: 60, time: 0, duration: 0.5, velocity: 0.8 });

    expect(midiToProject(bytesOf(midi), ".mid").project.name).toBe("音MAD");
    expect(midiToProject(bytesOf(midi), "no-extension").project.name).toBe("no-extension");
  });
});
