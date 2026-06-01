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
  /** Upper bound on frames analyzed; the hop widens so long clips don't block the caller. */
  maxScanFrames?: number;
}

const DEFAULT_THRESHOLD = 0.1;
// Accept pitches across the MIDI range the sample inspector lets users edit (24–96).
const DETECTABLE_MIDI_LOW = 24;
const DETECTABLE_MIDI_HIGH = 96;
// Frames are sized for the editable floor, but the search reaches a little lower
// so genuinely sub-editable samples are detected (and rejected) not pinned to it.
const EDITABLE_FLOOR_HZ = midiToFrequency(DETECTABLE_MIDI_LOW);
const DEFAULT_MIN_FREQUENCY = 20;
const DEFAULT_MAX_FREQUENCY = midiToFrequency(DETECTABLE_MIDI_HIGH);
const ENERGY_FLOOR = 1e-9;
const DEFAULT_MIN_PROBABILITY = 0.8;
const DEFAULT_MAX_SCAN_FRAMES = 256;

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
  if (denominator === 0) {
    // A flat parabola (e.g. an impulse frame's constant CMNDF) has no vertex to
    // interpolate; keep the integer lag rather than dividing by zero.
    return tau;
  }
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
  const half = tau >> 1;
  if (half >= 2 && cmndf[half]! < threshold) {
    // Half the detected period is itself a strong period above maxFrequency, so
    // the pick is an octave-down alias of an out-of-range fundamental; reject it.
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

/**
 * Find the start (on the hop grid) of the `scanLen`-sample window carrying the
 * most energy. A long clip can't be scanned in full within the frame budget, so
 * the bounded scan follows the audio here rather than truncating to the opening
 * seconds — letting it reach a note placed late, after a silent intro. The
 * sliding sum touches each sample a bounded number of times, so it stays O(n).
 */
function highestEnergyWindowStart(channel: Float32Array, hop: number, scanLen: number): number {
  const lastStart = channel.length - scanLen;
  let energy = 0;
  for (let i = 0; i < scanLen; i += 1) {
    energy += channel[i]! * channel[i]!;
  }
  let bestEnergy = energy;
  let bestStart = 0;
  for (let start = hop; start <= lastStart; start += hop) {
    for (let i = start - hop; i < start; i += 1) {
      energy -= channel[i]! * channel[i]!;
    }
    for (let i = start - hop + scanLen; i < start + scanLen; i += 1) {
      energy += channel[i]! * channel[i]!;
    }
    if (energy > bestEnergy) {
      bestEnergy = energy;
      bestStart = start;
    }
  }
  return bestStart;
}

/**
 * Estimate the recorded pitch of a decoded sample by running YIN across
 * overlapping frames and aggregating the voiced ones. The central pitch is the
 * median of voiced frames in the (log-frequency) MIDI domain, refined by a
 * confidence-weighted mean of the inliers within a semitone — robust against
 * attack transients, octave jumps, and noisy tails. Voiced frames are pooled
 * across every channel (not just the loudest), so a quiet-but-pitched channel
 * is still heard next to a louder unpitched one.
 */
export function detectSamplePitch(pcm: PcmAudio, options: DetectOptions = {}): SamplePitchEstimate | null {
  const first = pcm.channels[0];
  if (first === undefined || first.length === 0) {
    return null;
  }
  const minProbability = options.minProbability ?? DEFAULT_MIN_PROBABILITY;
  const maxScanFrames = options.maxScanFrames ?? DEFAULT_MAX_SCAN_FRAMES;
  const frameSize = Math.min(first.length, frameSizeFor(pcm.sampleRate, EDITABLE_FLOOR_HZ));
  // Keep the hop within one frame so windows stay contiguous (no gaps that skip
  // short notes), and split the frame budget across channels so total work stays
  // bounded regardless of channel count, even for silent or unpitched clips.
  const hop = Math.max(1, frameSize >> 2);
  const frameBudget = Math.max(1, Math.floor(maxScanFrames / pcm.channels.length));
  const scanLen = frameSize + hop * (frameBudget - 1);

  const midis: number[] = [];
  const probabilities: number[] = [];
  for (const channel of pcm.channels) {
    if (channel.length < frameSize) {
      continue;
    }
    const scanStart = channel.length <= scanLen ? 0 : highestEnergyWindowStart(channel, hop, scanLen);
    const scanEnd = Math.min(channel.length, scanStart + scanLen);
    for (let start = scanStart; start + frameSize <= scanEnd; start += hop) {
      const frame = channel.subarray(start, start + frameSize);
      const estimate = detectPitchYin(frame, pcm.sampleRate, options);
      if (estimate === null || estimate.probability < minProbability) {
        continue;
      }
      midis.push(frequencyToMidi(estimate.frequencyHz));
      probabilities.push(estimate.probability);
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
  const basePitch = Math.round(midi);
  if (basePitch < DETECTABLE_MIDI_LOW || basePitch > DETECTABLE_MIDI_HIGH) {
    // Outside the editable MIDI range (C1–C7); report no pitch rather than a
    // base pitch the inspector's slider cannot represent.
    return null;
  }
  // Playback adds tuneCents in pitchRatio, so store the correction that pulls
  // the sample onto basePitch (positive when the sample is flat of that note).
  const tuneCents = Math.round((basePitch - midi) * 100);
  return {
    frequencyHz: midiToFrequency(midi),
    midi,
    basePitch,
    tuneCents,
    probability: weight / inliers,
    voicedFrames: midis.length,
  };
}
