const NOTE_NAMES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"] as const;

export function semitonesToRatio(semitones: number): number {
  return Math.pow(2, semitones / 12);
}

/**
 * Playback speed ratio for a MIDI note played back from a sample recorded at
 * `basePitch`. A note one octave above the base plays at 2x speed.
 */
export function pitchRatio(notePitch: number, basePitch: number, tuneCents = 0): number {
  return semitonesToRatio(notePitch - basePitch + tuneCents / 100);
}

const A4_MIDI = 69;
const A4_HZ = 440;

/** Convert a frequency in Hz to a fractional MIDI note number (A4 = 440 Hz = 69). */
export function frequencyToMidi(frequencyHz: number): number {
  return A4_MIDI + 12 * Math.log2(frequencyHz / A4_HZ);
}

/** Convert a fractional MIDI note number to its frequency in Hz. */
export function midiToFrequency(midi: number): number {
  return A4_HZ * Math.pow(2, (midi - A4_MIDI) / 12);
}

export function midiToNoteName(midi: number): string {
  const clamped = Math.max(0, Math.min(127, Math.round(midi)));
  const name = NOTE_NAMES[clamped % 12]!;
  const octave = Math.floor(clamped / 12) - 1;
  return `${name}${octave}`;
}

export function noteNameToMidi(name: string): number | null {
  const match = /^([A-Ga-g])(#|b)?(-?\d+)$/.exec(name.trim());
  if (match === null) {
    return null;
  }
  const letter = match[1]!.toLowerCase();
  const accidental = match[2];
  const octaveRaw = match[3]!;
  const base: Record<string, number> = { c: 0, d: 2, e: 4, f: 5, g: 7, a: 9, b: 11 };
  let semitone = base[letter]!;
  if (accidental === "#") {
    semitone += 1;
  } else if (accidental === "b") {
    semitone -= 1;
  }
  const octave = Number.parseInt(octaveRaw, 10);
  return (octave + 1) * 12 + semitone;
}
