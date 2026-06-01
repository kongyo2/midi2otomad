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
  const now = ctx.currentTime;
  const attack = Math.max(0.001, sample.envelope.attackMs / 1000);
  const release = Math.max(0.001, sample.envelope.releaseMs / 1000);
  const hold = sample.loop.enabled ? 1.4 : Math.min(2.2, fullLength);
  gain.gain.setValueAtTime(0, now);
  gain.gain.linearRampToValueAtTime(sample.gain, now + attack);
  gain.gain.setValueAtTime(sample.gain, now + hold);
  gain.gain.linearRampToValueAtTime(0, now + hold + release);

  source.connect(gain);
  gain.connect(ctx.destination);
  source.start(now);
  source.stop(now + hold + release + 0.05);
}
