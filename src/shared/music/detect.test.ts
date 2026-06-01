import { describe, expect, it } from "vitest";
import { detectPitchYin, detectSamplePitch } from "./detect";
import type { PcmAudio } from "../audio/mixer";

function sine(frequencyHz: number, sampleRate: number, length: number, amplitude = 0.8): Float32Array {
  const out = new Float32Array(length);
  for (let i = 0; i < length; i += 1) {
    out[i] = amplitude * Math.sin((2 * Math.PI * frequencyHz * i) / sampleRate);
  }
  return out;
}

function centsOff(frequencyHz: number, referenceHz: number): number {
  return 1200 * Math.log2(frequencyHz / referenceHz);
}

function noise(length: number, seed = 1): Float32Array {
  const out = new Float32Array(length);
  let a = seed >>> 0;
  for (let i = 0; i < length; i += 1) {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    out[i] = (((t ^ (t >>> 14)) >>> 0) / 4294967296) * 2 - 1;
  }
  return out;
}

describe("detectPitchYin", () => {
  it("detects the fundamental of a 440 Hz sine within a few cents", () => {
    const sampleRate = 44100;
    const est = detectPitchYin(sine(440, sampleRate, 2048), sampleRate);
    expect(est).not.toBeNull();
    expect(Math.abs(centsOff(est!.frequencyHz, 440))).toBeLessThan(5);
    expect(est!.probability).toBeGreaterThan(0.9);
  });

  it("detects a low 220 Hz sine", () => {
    const sampleRate = 44100;
    const est = detectPitchYin(sine(220, sampleRate, 4096), sampleRate);
    expect(Math.abs(centsOff(est!.frequencyHz, 220))).toBeLessThan(5);
  });

  it("detects a high 880 Hz sine", () => {
    const sampleRate = 44100;
    const est = detectPitchYin(sine(880, sampleRate, 2048), sampleRate);
    expect(Math.abs(centsOff(est!.frequencyHz, 880))).toBeLessThan(5);
  });

  it("resolves a detuned frequency to sub-sample precision (parabolic interpolation)", () => {
    const sampleRate = 44100;
    // 443.7 Hz has a non-integer sample period, so nearest-bin matching would
    // be off by tens of cents; parabolic interpolation must recover it.
    const est = detectPitchYin(sine(443.7, sampleRate, 4096), sampleRate);
    expect(Math.abs(centsOff(est!.frequencyHz, 443.7))).toBeLessThan(2);
  });

  it("returns null for a silent frame", () => {
    expect(detectPitchYin(new Float32Array(2048), 44100)).toBeNull();
  });

  it("returns null for a frame too short for the search range", () => {
    expect(detectPitchYin(sine(440, 44100, 16), 44100)).toBeNull();
  });

  it("still yields a best-effort candidate via the global-minimum fallback when nothing crosses the threshold", () => {
    const sampleRate = 44100;
    // threshold 0 can never be crossed, so the absolute-threshold step always
    // defers to the global minimum of the search range.
    const est = detectPitchYin(sine(440, sampleRate, 2048), sampleRate, { threshold: 0 });
    expect(est).not.toBeNull();
    expect(Number.isFinite(est!.frequencyHz)).toBe(true);
    expect(est!.frequencyHz).toBeGreaterThan(0);
  });

  it("reports low probability for inharmonic noise", () => {
    const est = detectPitchYin(noise(2048), 44100);
    expect(est).not.toBeNull();
    expect(est!.probability).toBeLessThan(0.5);
  });

  it("clamps the estimate to the search range when the fundamental is below minFrequency", () => {
    const sampleRate = 44100;
    // True fundamental 1100 Hz sits below the 1150 Hz floor, so its period lies
    // beyond the lag range and the estimate is pinned to the searchable band.
    const est = detectPitchYin(sine(1100, sampleRate, 512), sampleRate, { minFrequency: 1150 });
    expect(est).not.toBeNull();
    expect(est!.frequencyHz).toBeGreaterThan(1150);
    expect(est!.frequencyHz).toBeLessThan(1250);
  });

  it("returns null for a constant (DC) frame with no periodicity", () => {
    const frame = new Float32Array(2048).fill(0.5);
    expect(detectPitchYin(frame, 44100)).toBeNull();
  });

  it("returns a finite estimate for an impulse frame whose CMNDF is flat", () => {
    // A single spike past the lag range makes the difference function constant,
    // so CMNDF is identically 1 and the parabola is flat (zero denominator).
    const frame = new Float32Array(64);
    frame[31] = 1;
    const est = detectPitchYin(frame, 1000, { minFrequency: 100, maxFrequency: 500 });
    expect(est).not.toBeNull();
    expect(Number.isFinite(est!.frequencyHz)).toBe(true);
  });
});

describe("detectSamplePitch", () => {
  it("detects the base pitch of a steady 440 Hz tone (A4 = MIDI 69)", () => {
    const sampleRate = 44100;
    const frames = sampleRate;
    const pcm: PcmAudio = { sampleRate, channels: [sine(440, sampleRate, frames)], frames };
    const est = detectSamplePitch(pcm);
    expect(est).not.toBeNull();
    expect(est!.basePitch).toBe(69);
    expect(Math.abs(est!.tuneCents)).toBeLessThan(5);
    expect(est!.voicedFrames).toBeGreaterThan(0);
    expect(est!.probability).toBeGreaterThan(0.9);
  });

  it("reports the residual detuning in cents for an off-center tone", () => {
    const sampleRate = 44100;
    const frames = sampleRate;
    // 256 Hz sits ~38 cents flat of C4 (261.63 Hz, MIDI 60).
    const pcm: PcmAudio = { sampleRate, channels: [sine(256, sampleRate, frames)], frames };
    const est = detectSamplePitch(pcm);
    expect(est!.basePitch).toBe(60);
    expect(est!.tuneCents).toBeLessThan(-30);
    expect(est!.tuneCents).toBeGreaterThan(-45);
  });

  it("returns null when there are no channels", () => {
    expect(detectSamplePitch({ sampleRate: 44100, channels: [], frames: 0 })).toBeNull();
  });

  it("returns null when the channel is empty", () => {
    expect(detectSamplePitch({ sampleRate: 44100, channels: [new Float32Array(0)], frames: 0 })).toBeNull();
  });

  it("detects the pitch despite a digital-silence lead-in", () => {
    const sampleRate = 44100;
    const frames = sampleRate;
    const lead = Math.floor(0.3 * sampleRate);
    const channel = new Float32Array(frames);
    channel.set(sine(440, sampleRate, frames - lead), lead);
    const est = detectSamplePitch({ sampleRate, channels: [channel], frames });
    expect(est!.basePitch).toBe(69);
  });

  it("returns null for inharmonic noise that never meets the confidence floor", () => {
    const sampleRate = 44100;
    const frames = sampleRate;
    const pcm: PcmAudio = { sampleRate, channels: [noise(frames)], frames };
    expect(detectSamplePitch(pcm, { minProbability: 0.99 })).toBeNull();
  });

  it("ignores a brief octave-jump glitch and keeps the dominant pitch", () => {
    const sampleRate = 44100;
    const frames = sampleRate;
    const channel = sine(440, sampleRate, frames);
    const glitchStart = Math.floor(0.4 * sampleRate);
    channel.set(sine(880, sampleRate, Math.floor(0.15 * sampleRate)), glitchStart);
    const est = detectSamplePitch({ sampleRate, channels: [channel], frames });
    expect(est!.basePitch).toBe(69);
  });

  it("detects a low bass tone in the editable range, below the old 55 Hz floor", () => {
    const sampleRate = 44100;
    const frames = sampleRate;
    // 49 Hz is G1 (MIDI 31); the inspector allows base pitches down to MIDI 24.
    const pcm: PcmAudio = { sampleRate, channels: [sine(49, sampleRate, frames)], frames };
    const est = detectSamplePitch(pcm);
    expect(est!.basePitch).toBe(31);
  });

  it("bounds the number of analyzed frames with an adaptive hop, even for unpitched clips", () => {
    const sampleRate = 44100;
    const frames = sampleRate;
    const pcm: PcmAudio = { sampleRate, channels: [sine(440, sampleRate, frames)], frames };
    // maxScanFrames 8 widens the hop so only 8 frames are scanned across the
    // whole clip, bounding work regardless of how many frames turn out voiced.
    const est = detectSamplePitch(pcm, { maxScanFrames: 8 });
    expect(est!.voicedFrames).toBe(8);
    expect(est!.basePitch).toBe(69);
  });

  it("mixes channels so a right-panned (left-silent) stereo tone is detected", () => {
    const sampleRate = 44100;
    const frames = sampleRate;
    const left = new Float32Array(frames);
    const right = sine(440, sampleRate, frames);
    const est = detectSamplePitch({ sampleRate, channels: [left, right], frames });
    expect(est!.basePitch).toBe(69);
  });

  it("returns null for a constant (DC) sample instead of emitting NaN", () => {
    const sampleRate = 44100;
    const frames = sampleRate;
    const pcm: PcmAudio = { sampleRate, channels: [new Float32Array(frames).fill(0.5)], frames };
    expect(detectSamplePitch(pcm)).toBeNull();
  });
});
