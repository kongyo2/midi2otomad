import { Midi } from "@tonejs/midi";
import { makeId } from "../../../shared/id";
import { type Polyphony, type Project, parseProject } from "../../../shared/schemas/project";

const TRACK_PALETTE = [
  "#7c5cff",
  "#36d399",
  "#f87272",
  "#fbbd23",
  "#3abff8",
  "#e879f9",
  "#f97316",
  "#22d3ee",
  "#a3e635",
  "#fb7185",
];

function clamp01(value: number): number {
  return Math.max(0, Math.min(1, value));
}

function stripExtension(fileName: string): string {
  return fileName.replace(/\.midi?$/i, "");
}

export interface MidiImportResult {
  project: Project;
  trackCount: number;
  noteCount: number;
}

/**
 * Parse a MIDI file into a project. The existing sample library, master gain,
 * sample rate, reverb bus and output settings are preserved so a user can
 * re-import arrangements without losing their material assignments and mix setup.
 */
export function midiToProject(bytes: Uint8Array, fileName: string, previous?: Project): MidiImportResult {
  const midi = new Midi(bytes);
  const header = midi.header;
  const previousSamples = previous?.samples ?? [];
  const fallbackSampleId = previousSamples[0]?.id ?? null;
  const previousSends = new Map<number, number>();
  const previousPolyphony = new Map<number, Polyphony>();
  for (const track of previous?.tracks ?? []) {
    if (track.midiIndex !== undefined) {
      previousSends.set(track.midiIndex, track.reverbSend);
      previousPolyphony.set(track.midiIndex, track.polyphony);
    }
  }

  let noteCount = 0;

  const tracks = midi.tracks
    .map((track, midiIndex) => ({ track, midiIndex }))
    .filter(({ track }) => track.notes.length > 0)
    .map(({ track, midiIndex }, index) => {
      const notes = track.notes.map((note) => {
        noteCount += 1;
        return {
          pitch: Math.max(0, Math.min(127, Math.round(note.midi))),
          startSec: Math.max(0, note.time),
          durationSec: Math.max(0.02, note.duration),
          velocity: Math.max(1, Math.min(127, Math.round(note.velocity * 127))),
        };
      });

      const volume = (track.controlChanges[7] ?? []).map((cc) => ({ t: Math.max(0, cc.time), v: clamp01(cc.value) }));
      const expression = (track.controlChanges[11] ?? []).map((cc) => ({
        t: Math.max(0, cc.time),
        v: clamp01(cc.value),
      }));

      const color = TRACK_PALETTE[index % TRACK_PALETTE.length]!;
      const name = track.name.trim() !== "" ? track.name.trim() : `Track ${index + 1}`;

      return {
        id: makeId("track"),
        name,
        midiIndex,
        color,
        muted: false,
        solo: false,
        gain: 1,
        pan: 0,
        defaultSampleId: fallbackSampleId,
        noteSampleMap: {},
        notes,
        dynamics: { volume, expression },
        reverbSend: previousSends.get(midiIndex) ?? 0,
        polyphony: previousPolyphony.get(midiIndex),
      };
    });

  const tempos = header.tempos.map((tempo) => ({
    timeSec: header.ticksToSeconds(tempo.ticks),
    bpm: tempo.bpm,
  }));

  const project = parseProject({
    version: 1,
    name: stripExtension(fileName) || "音MAD",
    bpm: header.tempos[0]?.bpm ?? previous?.bpm ?? 140,
    ppq: header.ppq,
    sampleRate: previous?.sampleRate ?? 48000,
    masterGain: previous?.masterGain ?? 1,
    reverb: previous?.reverb,
    output: previous?.output,
    tempos,
    samples: previousSamples,
    tracks,
  });

  return { project, trackCount: tracks.length, noteCount };
}
