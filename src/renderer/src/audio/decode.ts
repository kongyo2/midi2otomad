import type { PcmAudio } from "../../../shared/audio/mixer";
import { getAudioContext } from "./context";

/** Decode arbitrary audio bytes (wav/mp3/ogg/flac/…) into channel PCM. */
export async function decodeAudio(bytes: Uint8Array): Promise<PcmAudio> {
  const ctx = getAudioContext();
  const copy = bytes.slice();
  const audioBuffer = await ctx.decodeAudioData(copy.buffer);
  const channels: Float32Array[] = [];
  for (let c = 0; c < audioBuffer.numberOfChannels; c += 1) {
    channels.push(audioBuffer.getChannelData(c).slice());
  }
  if (channels.length === 0) {
    channels.push(new Float32Array(audioBuffer.length));
  }
  return { sampleRate: audioBuffer.sampleRate, channels, frames: audioBuffer.length };
}

/** A small downsampled magnitude envelope used to draw waveform thumbnails. */
export function buildWaveformPeaks(pcm: PcmAudio, buckets = 600): Float32Array {
  const peaks = new Float32Array(buckets);
  const channel = pcm.channels[0];
  if (channel === undefined || pcm.frames === 0) {
    return peaks;
  }
  const step = pcm.frames / buckets;
  for (let b = 0; b < buckets; b += 1) {
    const start = Math.floor(b * step);
    const end = Math.min(pcm.frames, Math.floor((b + 1) * step));
    let max = 0;
    for (let i = start; i < end; i += 1) {
      const v = Math.abs(channel[i] ?? 0);
      if (v > max) {
        max = v;
      }
    }
    peaks[b] = max;
  }
  return peaks;
}
