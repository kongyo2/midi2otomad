export interface MediaProbe {
  backend: "node-av";
  ffmpegVersion: string;
}

export type ExportFormat = "wav" | "mp3";
export type WavBitDepth = 16 | 24 | 32;

export interface BouncePcm {
  sampleRate: number;
  left: Float32Array;
  right: Float32Array;
  frames: number;
}

export interface BounceRequest {
  format: ExportFormat;
  pcm: BouncePcm;
  defaultName: string;
  wavBitDepth?: WavBitDepth;
  mp3Bitrate?: number;
}

export type BounceResponse =
  | { ok: true; path: string; bytes: number; durationSec: number }
  | { ok: false; canceled: boolean; error?: string };

export interface LoadedFile {
  name: string;
  data: Uint8Array;
}
