import type { Sample } from "../../../shared/schemas/project";
import type { PcmAudio } from "../../../shared/audio/mixer";
import { pitchRatio } from "../../../shared/music/pitch";
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
  const sustainLevel = peak * env.sustain;
  const onset = now + env.delayMs / 1000;
  const attack = Math.max(0.001, env.attackMs / 1000);
  const hold = env.holdMs / 1000;
  const decay = env.decayMs / 1000;
  const release = Math.max(0.001, env.releaseMs / 1000);
  const sustainHold = sample.loop.enabled ? 1.4 : Math.min(2.2, fullLength);

  const decayEnd = onset + attack + hold + decay;
  const sustainEnd = decayEnd + sustainHold;
  gain.gain.setValueAtTime(0, now);
  gain.gain.setValueAtTime(0, onset);
  gain.gain.linearRampToValueAtTime(peak, onset + attack);
  gain.gain.setValueAtTime(peak, onset + attack + hold);
  gain.gain.linearRampToValueAtTime(sustainLevel, decayEnd);
  gain.gain.setValueAtTime(sustainLevel, sustainEnd);
  gain.gain.linearRampToValueAtTime(0, sustainEnd + release);

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
  source.stop(sustainEnd + release + 0.05);
}
