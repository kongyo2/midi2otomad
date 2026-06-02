import { writeFile } from "node:fs/promises";
import type { BouncePcm, ExportFormat, WavBitDepth } from "../../shared/media";
import { resampleChannel } from "../../shared/audio/resample";

export type PcmInput = BouncePcm;

/** Sample rates libmp3lame accepts (MPEG-1 / 2 / 2.5). */
const MP3_SAMPLE_RATES = [8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000] as const;

/**
 * Map an arbitrary render rate onto a rate MP3 can encode. Rates that are an
 * exact power-of-two multiple of a supported rate fold back into that family
 * (96 kHz → 48 kHz, 88.2 kHz → 44.1 kHz); anything else snaps to the highest
 * supported rate at or below it, with the MP3 minimum as the floor.
 */
export function mp3CompatibleRate(rate: number): number {
  if ((MP3_SAMPLE_RATES as readonly number[]).includes(rate)) {
    return rate;
  }
  for (let i = MP3_SAMPLE_RATES.length - 1; i >= 0; i -= 1) {
    const supported = MP3_SAMPLE_RATES[i]!;
    const ratio = rate / supported;
    const rounded = Math.round(ratio);
    if (ratio > 1 && Math.abs(ratio - rounded) < 1e-9 && (rounded & (rounded - 1)) === 0) {
      return supported;
    }
  }
  for (let i = MP3_SAMPLE_RATES.length - 1; i >= 0; i -= 1) {
    if (MP3_SAMPLE_RATES[i]! <= rate) {
      return MP3_SAMPLE_RATES[i]!;
    }
  }
  return MP3_SAMPLE_RATES[0]!;
}

/** Materialise a channel to exactly `frames` samples, zero-padding any shortfall. */
function fullChannel(channel: Float32Array, frames: number): Float32Array {
  const out = new Float32Array(frames);
  out.set(channel.subarray(0, Math.min(channel.length, frames)));
  return out;
}

function resampleForMp3(pcm: PcmInput, dstRate: number): PcmInput {
  const left = resampleChannel(fullChannel(pcm.left, pcm.frames), pcm.sampleRate, dstRate);
  const right = resampleChannel(fullChannel(pcm.right, pcm.frames), pcm.sampleRate, dstRate);
  return { sampleRate: dstRate, left, right, frames: left.length };
}

export interface ExportRequest {
  format: ExportFormat;
  path: string;
  wavBitDepth?: WavBitDepth;
  mp3Bitrate?: number;
}

export interface ExportResult {
  path: string;
  bytes: number;
  durationSec: number;
}

function clamp(x: number): number {
  if (!Number.isFinite(x)) {
    return 0;
  }
  if (x > 1) {
    return 1;
  }
  if (x < -1) {
    return -1;
  }
  return x;
}

/** Build a complete RIFF/WAVE file buffer (PCM 16/24-bit or IEEE float 32-bit). */
export function encodeWav(pcm: PcmInput, bitDepth: WavBitDepth = 24): Buffer {
  const channels = 2;
  const { sampleRate, left, right, frames } = pcm;
  const bytesPerSample = bitDepth / 8;
  const blockAlign = channels * bytesPerSample;
  const dataSize = frames * blockAlign;
  const isFloat = bitDepth === 32;
  const fmtSize = isFloat ? 18 : 16;
  const factSize = isFloat ? 12 : 0;
  const headerSize = 12 + (8 + fmtSize) + factSize + 8;
  const buffer = Buffer.alloc(headerSize + dataSize);

  let p = 0;
  const writeTag = (tag: string): void => {
    buffer.write(tag, p, "ascii");
    p += 4;
  };
  const writeU32 = (value: number): void => {
    buffer.writeUInt32LE(value >>> 0, p);
    p += 4;
  };
  const writeU16 = (value: number): void => {
    buffer.writeUInt16LE(value & 0xffff, p);
    p += 2;
  };

  writeTag("RIFF");
  writeU32(headerSize - 8 + dataSize);
  writeTag("WAVE");

  writeTag("fmt ");
  writeU32(fmtSize);
  writeU16(isFloat ? 3 : 1);
  writeU16(channels);
  writeU32(sampleRate);
  writeU32(sampleRate * blockAlign);
  writeU16(blockAlign);
  writeU16(bitDepth);
  if (isFloat) {
    writeU16(0);
    writeTag("fact");
    writeU32(4);
    writeU32(frames);
  }

  writeTag("data");
  writeU32(dataSize);

  const dither = bitDepth === 16;
  const peakInt = bitDepth === 24 ? 0x7fffff : 0x7fff;
  for (let i = 0; i < frames; i += 1) {
    for (let c = 0; c < channels; c += 1) {
      const source = c === 0 ? left : right;
      const x = clamp(source[i] ?? 0);
      if (isFloat) {
        buffer.writeFloatLE(x, p);
        p += 4;
      } else {
        let scaled = x * peakInt;
        if (dither) {
          scaled += Math.random() - Math.random();
        }
        let v = Math.max(-peakInt - 1, Math.min(peakInt, Math.round(scaled)));
        if (bitDepth === 24) {
          if (v < 0) {
            v += 0x1000000;
          }
          buffer.writeUIntLE(v & 0xffffff, p, 3);
          p += 3;
        } else {
          buffer.writeInt16LE(v, p);
          p += 2;
        }
      }
    }
  }

  return buffer;
}

/** Encode the mix to MP3 via node-av (libmp3lame) and write it to disk. */
async function encodeMp3ToFile(input: PcmInput, outPath: string, kbps: number): Promise<void> {
  const targetRate = mp3CompatibleRate(input.sampleRate);
  const pcm = targetRate === input.sampleRate ? input : resampleForMp3(input, targetRate);
  const { Encoder, Muxer, Frame, FF_ENCODER_LIBMP3LAME, AV_SAMPLE_FMT_FLTP, AV_CHANNEL_LAYOUT_STEREO } =
    await import("node-av");
  const { sampleRate, left, right, frames } = pcm;
  const timeBase = { num: 1, den: sampleRate };

  const encoder = await Encoder.create(FF_ENCODER_LIBMP3LAME, { bitrate: `${kbps}k` });
  const muxer = await Muxer.open(outPath);
  const streamIndex = muxer.addStream(encoder);

  const FRAME_SAMPLES = 1152;

  async function* frames$(): AsyncGenerator<InstanceType<typeof Frame> | null> {
    let pts = 0n;
    for (let offset = 0; offset < frames; offset += FRAME_SAMPLES) {
      const n = Math.min(FRAME_SAMPLES, frames - offset);
      const planar = Buffer.allocUnsafe(n * 2 * 4);
      for (let i = 0; i < n; i += 1) {
        planar.writeFloatLE(clamp(left[offset + i] ?? 0), i * 4);
      }
      const rightBase = n * 4;
      for (let i = 0; i < n; i += 1) {
        planar.writeFloatLE(clamp(right[offset + i] ?? 0), rightBase + i * 4);
      }
      const frame = Frame.fromAudioBuffer(planar, {
        nbSamples: n,
        format: AV_SAMPLE_FMT_FLTP,
        sampleRate,
        channelLayout: AV_CHANNEL_LAYOUT_STEREO,
        timeBase,
        pts,
      });
      pts += BigInt(n);
      yield frame;
      frame.free();
    }
    yield null;
  }

  try {
    for await (const packet of encoder.packets(frames$())) {
      if (packet !== null) {
        await muxer.writePacket(packet, streamIndex);
        packet.free();
      }
    }
  } finally {
    await muxer.close();
    encoder.close();
  }
}

export async function writeExport(pcm: PcmInput, request: ExportRequest): Promise<ExportResult> {
  if (request.format === "wav") {
    const buffer = encodeWav(pcm, request.wavBitDepth ?? 24);
    await writeFile(request.path, buffer);
    return { path: request.path, bytes: buffer.byteLength, durationSec: pcm.frames / pcm.sampleRate };
  }
  await encodeMp3ToFile(pcm, request.path, request.mp3Bitrate ?? 320);
  const { stat } = await import("node:fs/promises");
  const info = await stat(request.path);
  return { path: request.path, bytes: info.size, durationSec: pcm.frames / pcm.sampleRate };
}
