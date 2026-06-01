import type { AutomationPoint, Note, Project, Sample, Track } from "../schemas/project";
import { pitchRatio } from "../music/pitch";

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

/** Resolve the effective loop region in source-sample units. */
function resolveLoop(sample: Sample, src: PcmAudio): { start: number; end: number; length: number } | null {
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

function readInterpolated(channel: Float32Array, pos: number, wrap: { start: number; length: number } | null): number {
  const i0 = Math.floor(pos);
  const frac = pos - i0;
  let a = channel[i0]!;
  let nextIndex = i0 + 1;
  if (wrap !== null && nextIndex >= wrap.start + wrap.length) {
    nextIndex = wrap.start + ((nextIndex - wrap.start) % wrap.length);
  }
  let b = channel[nextIndex]!;
  if (!Number.isFinite(a)) {
    a = 0;
  }
  if (!Number.isFinite(b)) {
    b = 0;
  }
  return a + (b - a) * frac;
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

function renderNote(
  note: Note,
  sample: Sample,
  src: PcmAudio,
  track: Track,
  trackDyn: Float32Array | null,
  left: Float32Array,
  right: Float32Array,
  outRate: number,
  masterGain: number,
  pan: { left: number; right: number },
): void {
  const totalFrames = left.length;
  const ratio = pitchRatio(note.pitch, sample.basePitch, sample.tuneCents);
  const increment = (src.sampleRate / outRate) * ratio;
  const startFrame = Math.round(note.startSec * outRate);
  const noteFrames = Math.max(1, Math.round(note.durationSec * outRate));
  const attackFrames = Math.max(1, Math.round((sample.envelope.attackMs / 1000) * outRate));
  const releaseFrames = Math.max(1, Math.round((sample.envelope.releaseMs / 1000) * outRate));
  const voiceFrames = noteFrames + releaseFrames;

  const velGain = velocityToGain(note.velocity);
  const staticGain = velGain * sample.gain * track.gain * masterGain;
  const loop = resolveLoop(sample, src);
  const wrap = loop === null ? null : { start: loop.start, length: loop.length };
  const ch0 = src.channels[0];
  if (ch0 === undefined) {
    return;
  }
  const ch1 = src.channels[1] ?? ch0;

  let srcPos = 0;
  for (let i = 0; i < voiceFrames; i += 1) {
    const outIdx = startFrame + i;
    if (outIdx < 0) {
      srcPos += increment;
      continue;
    }
    if (outIdx >= totalFrames) {
      break;
    }

    const attackGain = i < attackFrames ? i / attackFrames : 1;
    const releaseGain = i < noteFrames ? 1 : Math.max(0, 1 - (i - noteFrames) / releaseFrames);
    const env = attackGain * releaseGain;

    let pos = srcPos;
    let alive = true;
    if (loop !== null) {
      if (pos >= loop.end) {
        pos = loop.start + ((pos - loop.start) % loop.length);
      }
    } else if (pos >= src.frames - 1) {
      alive = false;
    }

    if (alive && env > 0) {
      const dyn = trackDyn === null ? 1 : trackDyn[outIdx]!;
      const amp = env * staticGain * dyn;
      const sL = readInterpolated(ch0, pos, wrap) * amp;
      const sR = readInterpolated(ch1, pos, wrap) * amp;
      left[outIdx] = left[outIdx]! + sL * pan.left;
      right[outIdx] = right[outIdx]! + sR * pan.right;
    }

    srcPos += increment;
  }
}

export function mixProject(project: Project, bank: AudioBank, options: MixOptions = {}): MixResult {
  const outRate = project.sampleRate;
  const sampleById = new Map<string, Sample>(project.samples.map((s) => [s.id, s]));
  const tailSec = options.tailSec ?? 0.25;
  const end = projectEndSeconds(project, sampleById) + tailSec;
  const frames = Math.max(MIN_FRAMES, Math.ceil(end * outRate) + 1);

  const left = new Float32Array(frames);
  const right = new Float32Array(frames);

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
      renderNote(note, sample, src, track, trackDyn, left, right, outRate, masterGain, pan);
    }
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
