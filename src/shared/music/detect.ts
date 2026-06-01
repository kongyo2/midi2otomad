import type { PcmAudio } from "../audio/mixer";
import { frequencyToMidi, midiToFrequency } from "./pitch";

export interface PitchEstimate {
  frequencyHz: number;
  probability: number;
}

export interface YinOptions {
  threshold?: number;
  minFrequency?: number;
  maxFrequency?: number;
}

export interface SamplePitchEstimate {
  frequencyHz: number;
  midi: number;
  basePitch: number;
  tuneCents: number;
  probability: number;
  voicedFrames: number;
}

export interface DetectOptions extends YinOptions {
  minProbability?: number;
  /** Stop after this many voiced frames so long clips don't block the caller. */
  maxVoicedFrames?: number;
}

const DEFAULT_THRESHOLD = 0.1;
// Detect across the same MIDI range the sample inspector lets users edit (24–96).
const DETECTABLE_MIDI_LOW = 24;
const DETECTABLE_MIDI_HIGH = 96;
const DEFAULT_MIN_FREQUENCY = midiToFrequency(DETECTABLE_MIDI_LOW);
const DEFAULT_MAX_FREQUENCY = midiToFrequency(DETECTABLE_MIDI_HIGH);
const ENERGY_FLOOR = 1e-9;
const DEFAULT_MIN_PROBABILITY = 0.8;
const DEFAULT_MAX_VOICED_FRAMES = 256;
const MIDI_MIN = 0;
const MIDI_MAX = 127;

function cumulativeMeanNormalizedDifference(frame: Float32Array, tauMax: number): Float32Array {
  const halfWindow = frame.length >> 1;
  const cmndf = new Float32Array(tauMax + 1);
  cmndf[0] = 1;
  let runningSum = 0;
  for (let tau = 1; tau <= tauMax; tau += 1) {
    let sum = 0;
    for (let j = 0; j < halfWindow; j += 1) {
      const diff = frame[j]! - frame[j + tau]!;
      sum += diff * diff;
    }
    runningSum += sum;
    cmndf[tau] = (sum * tau) / runningSum;
  }
  return cmndf;
}

function pickTau(cmndf: Float32Array, threshold: number, tauMin: number, searchEnd: number): number {
  for (let tau = tauMin; tau <= searchEnd; tau += 1) {
    if (cmndf[tau]! < threshold) {
      let t = tau;
      while (t < searchEnd && cmndf[t + 1]! < cmndf[t]!) {
        t += 1;
      }
      return t;
    }
  }
  let best = tauMin;
  for (let tau = tauMin + 1; tau <= searchEnd; tau += 1) {
    if (cmndf[tau]! < cmndf[best]!) {
      best = tau;
    }
  }
  return best;
}

function parabolicRefine(cmndf: Float32Array, tau: number): number {
  const s0 = cmndf[tau - 1]!;
  const s1 = cmndf[tau]!;
  const s2 = cmndf[tau + 1]!;
  const denominator = 2 * s1 - s0 - s2;
  // The vertex of a true minimum lies within half a bin; clamp to the adjacent
  // bins so a monotonic slope at the search boundary cannot extrapolate away.
  const shift = Math.max(-1, Math.min(1, (s2 - s0) / (2 * denominator)));
  return tau + shift;
}

/**
 * Estimate the fundamental frequency of a single frame with the YIN algorithm
 * (de Cheveigné & Kawahara, 2002): cumulative-mean-normalized difference plus
 * parabolic interpolation for sub-sample-period (sub-cent) precision.
 */
export function detectPitchYin(
  frame: Float32Array,
  sampleRate: number,
  options: YinOptions = {},
): PitchEstimate | null {
  const threshold = options.threshold ?? DEFAULT_THRESHOLD;
  const minFrequency = options.minFrequency ?? DEFAULT_MIN_FREQUENCY;
  const maxFrequency = options.maxFrequency ?? DEFAULT_MAX_FREQUENCY;

  const halfWindow = frame.length >> 1;
  const tauMax = Math.min(halfWindow, Math.floor(sampleRate / minFrequency));
  const tauMin = Math.max(1, Math.floor(sampleRate / maxFrequency));
  const searchEnd = tauMax - 1;
  if (searchEnd < tauMin) {
    return null;
  }

  let energy = 0;
  for (let i = 0; i < frame.length; i += 1) {
    energy += frame[i]! * frame[i]!;
  }
  if (energy <= ENERGY_FLOOR) {
    return null;
  }

  const cmndf = cumulativeMeanNormalizedDifference(frame, tauMax);
  const tau = pickTau(cmndf, threshold, tauMin, searchEnd);
  if (!Number.isFinite(cmndf[tau]!)) {
    // A constant/DC frame yields an all-zero difference function, normalizing to
    // NaN; treat it as unpitched rather than leaking NaN into the estimate.
    return null;
  }
  const refinedTau = parabolicRefine(cmndf, tau);
  const frequencyHz = sampleRate / refinedTau;
  const probability = Math.max(0, Math.min(1, 1 - cmndf[tau]!));
  return { frequencyHz, probability };
}

function frameSizeFor(sampleRate: number, minFrequency: number): number {
  const needed = (sampleRate / minFrequency) * 2;
  const power = Math.ceil(Math.log2(Math.max(needed, 1)));
  return 2 ** Math.max(11, power);
}

function median(values: number[]): number {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[sorted.length >> 1]!;
}

function toMono(channels: Float32Array[], frames: number): Float32Array {
  if (channels.length === 1) {
    return channels[0]!;
  }
  const mono = new Float32Array(frames);
  for (const channel of channels) {
    for (let i = 0; i < frames; i += 1) {
      mono[i]! += channel[i]! / channels.length;
    }
  }
  return mono;
}

/**
 * Estimate the recorded pitch of a decoded sample by running YIN across
 * overlapping frames and aggregating the voiced ones. The central pitch is the
 * median of voiced frames in the (log-frequency) MIDI domain, refined by a
 * confidence-weighted mean of the inliers within a semitone — robust against
 * attack transients, octave jumps, and noisy tails.
 */
export function detectSamplePitch(pcm: PcmAudio, options: DetectOptions = {}): SamplePitchEstimate | null {
  const first = pcm.channels[0];
  if (first === undefined || first.length === 0) {
    return null;
  }
  const channel = toMono(pcm.channels, first.length);
  const minFrequency = options.minFrequency ?? DEFAULT_MIN_FREQUENCY;
  const minProbability = options.minProbability ?? DEFAULT_MIN_PROBABILITY;
  const maxVoicedFrames = options.maxVoicedFrames ?? DEFAULT_MAX_VOICED_FRAMES;
  const frameSize = Math.min(channel.length, frameSizeFor(pcm.sampleRate, minFrequency));
  const hop = Math.max(1, frameSize >> 2);

  const midis: number[] = [];
  const probabilities: number[] = [];
  for (let start = 0; start + frameSize <= channel.length; start += hop) {
    const frame = channel.subarray(start, start + frameSize);
    const estimate = detectPitchYin(frame, pcm.sampleRate, options);
    if (estimate === null || estimate.probability < minProbability) {
      continue;
    }
    midis.push(frequencyToMidi(estimate.frequencyHz));
    probabilities.push(estimate.probability);
    if (midis.length >= maxVoicedFrames) {
      break;
    }
  }
  if (midis.length === 0) {
    return null;
  }

  const center = median(midis);
  let weightedMidi = 0;
  let weight = 0;
  let inliers = 0;
  for (let i = 0; i < midis.length; i += 1) {
    if (Math.abs(midis[i]! - center) <= 0.5) {
      weightedMidi += midis[i]! * probabilities[i]!;
      weight += probabilities[i]!;
      inliers += 1;
    }
  }

  const midi = weightedMidi / weight;
  const basePitch = Math.max(MIDI_MIN, Math.min(MIDI_MAX, Math.round(midi)));
  const tuneCents = Math.round((midi - basePitch) * 100);
  return {
    frequencyHz: midiToFrequency(midi),
    midi,
    basePitch,
    tuneCents,
    probability: weight / inliers,
    voicedFrames: midis.length,
  };
}
