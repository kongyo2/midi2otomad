import type { AutomationPoint, Note, Project, Sample, Track } from "../schemas/project";
import { pitchRatio, semitonesToRatio } from "../music/pitch";
import { cubicHermite } from "./interpolation";
import { envelopeLevel } from "./envelope";
import { pitchOffsetSemitones } from "./pitchmod";
import { createBiquadState, designBiquad, processBiquadSample, type BiquadCoeffs } from "./filter";
import { createReverb } from "./reverb";

/** Decoded source material: one Float32Array per channel, all the same length. */
export interface PcmAudio {
  sampleRate: number;
  channels: Float32Array[];
  frames: number;
}

export interface AudioBank {
  get(id: string): PcmAudio | undefined;
}

export function bankFromRecord(record: Record<string, PcmAudio>): AudioBank {
  return { get: (id) => record[id] };
}

export interface MixResult {
  sampleRate: number;
  left: Float32Array;
  right: Float32Array;
  frames: number;
  durationSec: number;
  peak: number;
}

export interface MixOptions {
  /** Extra silence appended after the last sounding sample, in seconds. */
  tailSec?: number;
  /** Apply the soft-knee limiter on the master bus. */
  limiter?: boolean;
}

const MIN_FRAMES = 1;

export function velocityToGain(velocity: number): number {
  const v = Math.max(0, Math.min(127, velocity)) / 127;
  // Slight curve so soft notes feel softer without losing the loud end.
  return Math.pow(v, 1.35);
}

function sampleAutomation(points: AutomationPoint[], t: number): number {
  const first = points[0];
  if (first === undefined || t < first.t) {
    // Before the first explicit controller event the value defaults to full,
    // rather than projecting the first (possibly low) event backward over the intro.
    return 1;
  }
  let prev = first;
  for (let i = 1; i < points.length; i += 1) {
    const next = points[i]!;
    if (t < next.t) {
      const span = next.t - prev.t;
      return prev.v + (next.v - prev.v) * ((t - prev.t) / span);
    }
    prev = next;
  }
  return prev.v;
}

interface LoopRegion {
  start: number;
  end: number;
  length: number;
}

/** Resolve the effective loop region in source-sample units. */
function resolveLoop(sample: Sample, src: PcmAudio): LoopRegion | null {
  if (!sample.loop.enabled) {
    return null;
  }
  const start = Math.max(0, Math.floor(sample.loop.startSec * src.sampleRate));
  const rawEnd = sample.loop.endSec > sample.loop.startSec ? sample.loop.endSec : src.frames / src.sampleRate;
  const end = Math.min(src.frames, Math.floor(rawEnd * src.sampleRate));
  const length = end - start;
  if (length < 2) {
    return null;
  }
  return { start, end, length };
}

/** Fetch a single source sample, wrapping inside the loop when periodic and clamping otherwise. */
function sampleAt(channel: Float32Array, frames: number, index: number, region: LoopRegion | null): number {
  let idx: number;
  if (region !== null) {
    idx = region.start + ((((index - region.start) % region.length) + region.length) % region.length);
  } else {
    idx = index < 0 ? 0 : index >= frames ? frames - 1 : index;
  }
  const value = channel[idx]!;
  return Number.isFinite(value) ? value : 0;
}

/** Read a (possibly fractional) source position with linear or cubic-hermite interpolation. */
function readSample(
  channel: Float32Array,
  frames: number,
  pos: number,
  hermite: boolean,
  region: LoopRegion | null,
): number {
  const i0 = Math.floor(pos);
  const frac = pos - i0;
  if (!hermite) {
    const a = sampleAt(channel, frames, i0, region);
    const b = sampleAt(channel, frames, i0 + 1, region);
    return a + (b - a) * frac;
  }
  const y0 = sampleAt(channel, frames, i0 - 1, region);
  const y1 = sampleAt(channel, frames, i0, region);
  const y2 = sampleAt(channel, frames, i0 + 1, region);
  const y3 = sampleAt(channel, frames, i0 + 2, region);
  return cubicHermite(y0, y1, y2, y3, frac);
}

function softClip(x: number): number {
  const threshold = 0.8;
  const abs = Math.abs(x);
  if (abs <= threshold) {
    return x;
  }
  const sign = x < 0 ? -1 : 1;
  const over = (abs - threshold) / (1 - threshold);
  return sign * (threshold + (1 - threshold) * Math.tanh(over));
}

function panGains(pan: number): { left: number; right: number } {
  const p = Math.max(-1, Math.min(1, pan));
  return {
    left: p <= 0 ? 1 : 1 - p,
    right: p >= 0 ? 1 : 1 + p,
  };
}

function anySolo(tracks: Track[]): boolean {
  return tracks.some((t) => t.solo);
}

function projectEndSeconds(project: Project, sampleById: Map<string, Sample>): number {
  let end = 0;
  for (const track of project.tracks) {
    for (const note of track.notes) {
      const sampleId = track.noteSampleMap[String(note.pitch)] ?? track.defaultSampleId;
      const release = sampleId !== null ? (sampleById.get(sampleId)?.envelope.releaseMs ?? 0) : 0;
      const tail = note.startSec + note.durationSec + release / 1000;
      if (tail > end) {
        end = tail;
      }
    }
  }
  return end;
}

function buildTrackDynamics(track: Track, frames: number, sampleRate: number): Float32Array | null {
  const { volume, expression } = track.dynamics;
  if (volume.length === 0 && expression.length === 0) {
    return null;
  }
  const out = new Float32Array(frames);
  for (let i = 0; i < frames; i += 1) {
    const t = i / sampleRate;
    out[i] = sampleAutomation(volume, t) * sampleAutomation(expression, t);
  }
  return out;
}

interface SendBus {
  l: Float32Array;
  r: Float32Array;
}

interface Buses {
  left: Float32Array;
  right: Float32Array;
  send: SendBus | null;
}

function renderNote(
  note: Note,
  sample: Sample,
  src: PcmAudio,
  track: Track,
  trackDyn: Float32Array | null,
  buses: Buses,
  outRate: number,
  masterGain: number,
  pan: { left: number; right: number },
): void {
  const total = buses.left.length;
  const baseRatio = pitchRatio(note.pitch, sample.basePitch, sample.tuneCents);
  const baseIncrement = (src.sampleRate / outRate) * baseRatio;
  const startFrame = Math.round(note.startSec * outRate);
  const noteFrames = Math.max(1, Math.round(note.durationSec * outRate));
  const releaseFrames = Math.max(0, Math.round((sample.envelope.releaseMs / 1000) * outRate));
  const voiceFrames = noteFrames + releaseFrames;
  const gateSec = note.durationSec;

  const velGain = velocityToGain(note.velocity);
  const staticGain = velGain * sample.gain * track.gain * masterGain;
  const loop = resolveLoop(sample, src);
  const hermite = sample.interpolation === "hermite";

  const ch0 = src.channels[0];
  if (ch0 === undefined) {
    return;
  }
  const ch1 = src.channels[1] ?? ch0;

  const coeffs: BiquadCoeffs | null = sample.filter.enabled
    ? designBiquad(sample.filter.type, sample.filter.cutoffHz, outRate, sample.filter.q, sample.filter.gainDb)
    : null;
  const stateL = createBiquadState();
  const stateR = createBiquadState();
  const send = buses.send;
  const reverbSend = track.reverbSend;

  let srcPos = 0;
  for (let i = 0; i < voiceFrames; i += 1) {
    const outIdx = startFrame + i;
    const tSec = i / outRate;
    const increment = baseIncrement * semitonesToRatio(pitchOffsetSemitones(sample.pitchMod, tSec));
    if (outIdx < 0) {
      srcPos += increment;
      continue;
    }
    if (outIdx >= total) {
      break;
    }

    let pos = srcPos;
    let alive = true;
    let region: LoopRegion | null = null;
    if (loop !== null) {
      if (pos >= loop.end) {
        pos = loop.start + ((pos - loop.start) % loop.length);
      }
      if (pos >= loop.start) {
        region = loop;
      }
    } else if (pos >= src.frames - 1) {
      alive = false;
    }

    if (alive) {
      let sL = readSample(ch0, src.frames, pos, hermite, region);
      let sR = readSample(ch1, src.frames, pos, hermite, region);
      if (coeffs !== null) {
        sL = processBiquadSample(coeffs, stateL, sL);
        sR = processBiquadSample(coeffs, stateR, sR);
      }
      const env = envelopeLevel(sample.envelope, tSec, gateSec);
      if (env > 0) {
        const dyn = trackDyn === null ? 1 : trackDyn[outIdx]!;
        const amp = env * staticGain * dyn;
        const outL = sL * amp * pan.left;
        const outR = sR * amp * pan.right;
        buses.left[outIdx] = buses.left[outIdx]! + outL;
        buses.right[outIdx] = buses.right[outIdx]! + outR;
        if (send !== null && reverbSend > 0) {
          send.l[outIdx] = send.l[outIdx]! + outL * reverbSend;
          send.r[outIdx] = send.r[outIdx]! + outR * reverbSend;
        }
      }
    }

    srcPos += increment;
  }
}

function reverbTailSeconds(project: Project): number {
  const r = project.reverb;
  if (!r.enabled) {
    return 0;
  }
  return 0.5 + r.roomSize * 4 + r.preDelayMs / 1000;
}

function applyReverb(project: Project, outRate: number, send: SendBus, left: Float32Array, right: Float32Array): void {
  const r = project.reverb;
  const verb = createReverb(outRate, {
    roomSize: r.roomSize,
    damping: r.damping,
    width: r.width,
    wet: r.wet,
    dry: 0,
    preDelayMs: r.preDelayMs,
  });
  const wet = verb.processBlock(send.l, send.r);
  for (let i = 0; i < left.length; i += 1) {
    left[i] = left[i]! + wet.left[i]!;
    right[i] = right[i]! + wet.right[i]!;
  }
}

export function mixProject(project: Project, bank: AudioBank, options: MixOptions = {}): MixResult {
  const outRate = project.sampleRate;
  const sampleById = new Map<string, Sample>(project.samples.map((s) => [s.id, s]));
  const tailSec = options.tailSec ?? 0.25;
  const end = projectEndSeconds(project, sampleById) + tailSec + reverbTailSeconds(project);
  const frames = Math.max(MIN_FRAMES, Math.ceil(end * outRate) + 1);

  const left = new Float32Array(frames);
  const right = new Float32Array(frames);
  const send: SendBus | null = project.reverb.enabled
    ? { l: new Float32Array(frames), r: new Float32Array(frames) }
    : null;
  const buses: Buses = { left, right, send };

  const solo = anySolo(project.tracks);
  const masterGain = project.masterGain;

  for (const track of project.tracks) {
    if (track.muted || (solo && !track.solo)) {
      continue;
    }
    const pan = panGains(track.pan);
    const trackDyn = buildTrackDynamics(track, frames, outRate);
    for (const note of track.notes) {
      const sampleId = track.noteSampleMap[String(note.pitch)] ?? track.defaultSampleId;
      if (sampleId === null) {
        continue;
      }
      const sample = sampleById.get(sampleId);
      const src = bank.get(sampleId);
      if (sample === undefined || src === undefined || src.frames < 2) {
        continue;
      }
      renderNote(note, sample, src, track, trackDyn, buses, outRate, masterGain, pan);
    }
  }

  if (send !== null) {
    applyReverb(project, outRate, send, left, right);
  }

  let peak = 0;
  for (let i = 0; i < frames; i += 1) {
    const l = left[i]!;
    const r = right[i]!;
    const m = Math.max(Math.abs(l), Math.abs(r));
    if (m > peak) {
      peak = m;
    }
  }

  if (options.limiter !== false) {
    for (let i = 0; i < frames; i += 1) {
      left[i] = softClip(left[i]!);
      right[i] = softClip(right[i]!);
    }
  }

  return { sampleRate: outRate, left, right, frames, durationSec: frames / outRate, peak };
}
