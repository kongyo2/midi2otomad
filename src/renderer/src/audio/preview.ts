import type { Sample } from "../../../shared/schemas/project";
import type { PcmAudio } from "../../../shared/audio/mixer";
import { pitchRatio } from "../../../shared/music/pitch";
import { envelopeLevel } from "../../../shared/audio/envelope";
import { getAudioContext, resumeAudioContext } from "./context";

/** Audition a single material one-shot at the given pitch (defaults to base pitch). */
export function previewSample(pcm: PcmAudio, sample: Sample, pitch?: number): void {
  const ctx = getAudioContext();
  void resumeAudioContext();
  const buffer = ctx.createBuffer(pcm.channels.length, pcm.frames, pcm.sampleRate);
  pcm.channels.forEach((channel, index) => buffer.copyToChannel(channel as Float32Array<ArrayBuffer>, index));

  const source = ctx.createBufferSource();
  source.buffer = buffer;
  source.playbackRate.value = pitchRatio(pitch ?? sample.basePitch, sample.basePitch, sample.tuneCents);

  const fullLength = pcm.frames / pcm.sampleRate;
  if (sample.loop.enabled) {
    source.loop = true;
    source.loopStart = sample.loop.startSec;
    source.loopEnd = sample.loop.endSec > sample.loop.startSec ? sample.loop.endSec : fullLength;
  }

  const gain = ctx.createGain();
  const env = sample.envelope;
  const now = ctx.currentTime;
  const peak = sample.gain;
  // Audition the exact engine envelope by sampling envelopeLevel into a value
  // curve, so attack/decay/release shapes match playback and export.
  const sustainHold = sample.loop.enabled ? 1.4 : Math.min(2.2, fullLength);
  const gateSec = (env.delayMs + env.attackMs + env.holdMs + env.decayMs) / 1000 + sustainHold;
  const total = gateSec + env.releaseMs / 1000;
  const points = Math.max(2, Math.min(8000, Math.round(total * 1000)));
  const curve = new Float32Array(points);
  for (let i = 0; i < points; i += 1) {
    const t = (i / (points - 1)) * total;
    curve[i] = envelopeLevel(env, t, gateSec) * peak;
  }
  gain.gain.setValueCurveAtTime(curve, now, total);

  if (sample.filter.enabled) {
    const filter = ctx.createBiquadFilter();
    filter.type = sample.filter.type;
    filter.frequency.value = sample.filter.cutoffHz;
    filter.Q.value = sample.filter.q;
    filter.gain.value = sample.filter.gainDb;
    source.connect(filter);
    filter.connect(gain);
  } else {
    source.connect(gain);
  }
  gain.connect(ctx.destination);
  source.start(now);
  source.stop(now + total + 0.05);
}
