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

export function midiToNoteName(midi: number): string {
  const clamped = Math.max(0, Math.min(127, Math.round(midi)));
  const name = NOTE_NAMES[clamped % 12] ?? "C";
  const octave = Math.floor(clamped / 12) - 1;
  return `${name}${octave}`;
}

export function noteNameToMidi(name: string): number | null {
  const match = /^([A-Ga-g])(#|b)?(-?\d+)$/.exec(name.trim());
  if (match === null) {
    return null;
  }
  const [, letter, accidental, octaveRaw] = match;
  const base: Record<string, number> = { c: 0, d: 2, e: 4, f: 5, g: 7, a: 9, b: 11 };
  let semitone = base[(letter ?? "c").toLowerCase()] ?? 0;
  if (accidental === "#") {
    semitone += 1;
  } else if (accidental === "b") {
    semitone -= 1;
  }
  const octave = Number.parseInt(octaveRaw ?? "0", 10);
  return (octave + 1) * 12 + semitone;
}
