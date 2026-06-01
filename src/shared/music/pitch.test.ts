import { describe, expect, it } from "vitest";
import {
  frequencyToMidi,
  midiToFrequency,
  midiToNoteName,
  noteNameToMidi,
  pitchRatio,
  semitonesToRatio,
} from "./pitch";

describe("semitonesToRatio", () => {
  it("maps 12 semitones to one octave (2x)", () => {
    expect(semitonesToRatio(12)).toBeCloseTo(2, 9);
  });

  it("maps 0 semitones to unison (1x)", () => {
    expect(semitonesToRatio(0)).toBe(1);
  });

  it("maps -12 semitones to half speed", () => {
    expect(semitonesToRatio(-12)).toBeCloseTo(0.5, 9);
  });
});

describe("pitchRatio", () => {
  it("plays one octave up at double speed", () => {
    expect(pitchRatio(72, 60)).toBeCloseTo(2, 9);
  });

  it("plays the base pitch at unison", () => {
    expect(pitchRatio(60, 60)).toBeCloseTo(1, 9);
  });

  it("applies tuning offset in cents", () => {
    expect(pitchRatio(60, 60, 1200)).toBeCloseTo(2, 9);
  });

  it("defaults tuning to zero cents", () => {
    expect(pitchRatio(63, 60)).toBeCloseTo(semitonesToRatio(3), 9);
  });
});

describe("midiToNoteName", () => {
  it("names middle C as C4", () => {
    expect(midiToNoteName(60)).toBe("C4");
  });

  it("names accidentals with sharps", () => {
    expect(midiToNoteName(61)).toBe("C#4");
    expect(midiToNoteName(69)).toBe("A4");
  });

  it("clamps values below 0", () => {
    expect(midiToNoteName(-5)).toBe("C-1");
  });

  it("clamps values above 127", () => {
    expect(midiToNoteName(200)).toBe("G9");
  });

  it("rounds fractional midi numbers", () => {
    expect(midiToNoteName(60.4)).toBe("C4");
    expect(midiToNoteName(60.6)).toBe("C#4");
  });
});

describe("frequencyToMidi", () => {
  it("maps concert A (440 Hz) to MIDI 69", () => {
    expect(frequencyToMidi(440)).toBeCloseTo(69, 9);
  });

  it("maps one octave up to twelve semitones up", () => {
    expect(frequencyToMidi(880)).toBeCloseTo(81, 9);
  });

  it("maps one octave down to twelve semitones down", () => {
    expect(frequencyToMidi(220)).toBeCloseTo(57, 9);
  });
});

describe("midiToFrequency", () => {
  it("maps MIDI 69 to concert A (440 Hz)", () => {
    expect(midiToFrequency(69)).toBeCloseTo(440, 9);
  });

  it("maps twelve semitones up to one octave up", () => {
    expect(midiToFrequency(81)).toBeCloseTo(880, 9);
  });

  it("round-trips with frequencyToMidi across the range", () => {
    for (let midi = 24; midi <= 108; midi += 1) {
      expect(frequencyToMidi(midiToFrequency(midi))).toBeCloseTo(midi, 9);
    }
  });
});

describe("noteNameToMidi", () => {
  it("parses plain note names", () => {
    expect(noteNameToMidi("C4")).toBe(60);
    expect(noteNameToMidi("A4")).toBe(69);
  });

  it("parses sharps", () => {
    expect(noteNameToMidi("C#4")).toBe(61);
  });

  it("parses flats", () => {
    expect(noteNameToMidi("Db4")).toBe(61);
    expect(noteNameToMidi("Cb4")).toBe(59);
  });

  it("accepts lowercase note letters", () => {
    expect(noteNameToMidi("g3")).toBe(55);
  });

  it("trims surrounding whitespace", () => {
    expect(noteNameToMidi("  C4 ")).toBe(60);
  });

  it("parses negative octaves", () => {
    expect(noteNameToMidi("C-1")).toBe(0);
  });

  it("returns null for invalid input", () => {
    expect(noteNameToMidi("H4")).toBeNull();
    expect(noteNameToMidi("C")).toBeNull();
    expect(noteNameToMidi("")).toBeNull();
  });

  it("round-trips with midiToNoteName", () => {
    for (let midi = 12; midi <= 120; midi += 1) {
      expect(noteNameToMidi(midiToNoteName(midi))).toBe(midi);
    }
  });
});
