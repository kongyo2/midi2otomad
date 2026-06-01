import type { MixResult } from "../../../shared/audio/mixer";
import { getAudioContext, resumeAudioContext } from "./context";

export type TransportState = "stopped" | "playing" | "paused";

/**
 * Plays a fully pre-rendered stereo mix as a single buffer. Because the whole
 * song is rendered ahead of time (by the shared mixer), seeking and pausing are
 * just offset bookkeeping, and what you hear is identical to the exported file.
 */
export class PreviewEngine {
  private readonly ctx: AudioContext;
  private readonly master: GainNode;
  private readonly analyser: AnalyserNode;
  private buffer: AudioBuffer | null = null;
  private source: AudioBufferSourceNode | null = null;
  private startCtxTime = 0;
  private offset = 0;
  private state: TransportState = "stopped";
  private stopping = false;

  onEnded: (() => void) | null = null;

  constructor() {
    this.ctx = getAudioContext();
    this.master = this.ctx.createGain();
    this.analyser = this.ctx.createAnalyser();
    this.analyser.fftSize = 1024;
    this.master.connect(this.analyser);
    this.analyser.connect(this.ctx.destination);
  }

  get transport(): TransportState {
    return this.state;
  }

  get durationSec(): number {
    return this.buffer?.duration ?? 0;
  }

  setMix(mix: MixResult): void {
    const wasPlaying = this.state === "playing";
    const resumeAt = this.getPosition();
    const buffer = this.ctx.createBuffer(2, Math.max(1, mix.frames), mix.sampleRate);
    buffer.copyToChannel(mix.left as Float32Array<ArrayBuffer>, 0);
    buffer.copyToChannel(mix.right as Float32Array<ArrayBuffer>, 1);
    this.buffer = buffer;
    if (wasPlaying) {
      this.play(resumeAt);
    } else {
      this.offset = Math.min(resumeAt, buffer.duration);
    }
  }

  getMasterAnalyser(): AnalyserNode {
    return this.analyser;
  }

  getPosition(): number {
    if (this.state === "playing") {
      const pos = this.offset + (this.ctx.currentTime - this.startCtxTime);
      return Math.min(pos, this.durationSec);
    }
    return this.offset;
  }

  setMasterGain(value: number): void {
    this.master.gain.value = value;
  }

  private teardownSource(): void {
    if (this.source !== null) {
      this.stopping = true;
      try {
        this.source.onended = null;
        this.source.stop();
      } catch {
        // already stopped
      }
      this.source.disconnect();
      this.source = null;
      this.stopping = false;
    }
  }

  play(fromSec?: number): void {
    if (this.buffer === null) {
      return;
    }
    void resumeAudioContext();
    this.teardownSource();
    const start = Math.max(0, Math.min(fromSec ?? this.offset, this.buffer.duration));
    const source = this.ctx.createBufferSource();
    source.buffer = this.buffer;
    source.connect(this.master);
    source.onended = () => {
      if (this.stopping) {
        return;
      }
      this.state = "stopped";
      this.offset = 0;
      this.source = null;
      this.onEnded?.();
    };
    source.start(0, start);
    this.source = source;
    this.startCtxTime = this.ctx.currentTime;
    this.offset = start;
    this.state = "playing";
  }

  pause(): void {
    if (this.state !== "playing") {
      return;
    }
    this.offset = this.getPosition();
    this.teardownSource();
    this.state = "paused";
  }

  stop(): void {
    this.teardownSource();
    this.offset = 0;
    this.state = "stopped";
  }

  seek(sec: number): void {
    const clamped = Math.max(0, Math.min(sec, this.durationSec || sec));
    if (this.state === "playing") {
      this.play(clamped);
    } else {
      this.offset = clamped;
    }
  }
}
