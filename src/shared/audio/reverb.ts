/**
 * A faithful Freeverb (Jezar's public-domain Schroeder/Moorer reverb): eight
 * damped feedback-comb filters in parallel feeding four series allpass
 * diffusers, run as an independent pair for the left and right channels with a
 * small delay spread between them for a wide stereo image. A pre-delay line in
 * front of the network pushes the reverb tail back behind the dry signal.
 */
export interface ReverbParams {
  /** Tail length, 0..1. Maps to comb feedback between 0.7 and 0.98. */
  roomSize: number;
  /** High-frequency absorption in the tail, 0..1. */
  damping: number;
  /** Stereo width of the wet signal, 0 (mono) .. 1 (fully spread). */
  width: number;
  /** Wet (reverberated) level. */
  wet: number;
  /** Dry (unprocessed) level passed straight through. */
  dry: number;
  /** Delay before the wet tail begins, in milliseconds. */
  preDelayMs: number;
}

export interface ReverbOutput {
  left: Float32Array;
  right: Float32Array;
}

export interface Reverb {
  processBlock(left: Float32Array, right: Float32Array): ReverbOutput;
}

const COMB_TUNINGS = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASS_TUNINGS = [556, 441, 341, 225];
const STEREO_SPREAD = 23;
const FIXED_GAIN = 0.015;
const SCALE_ROOM = 0.28;
const OFFSET_ROOM = 0.7;
const SCALE_DAMP = 0.4;
const ALLPASS_FEEDBACK = 0.5;

class Comb {
  private readonly buffer: Float32Array;
  private index = 0;
  private store = 0;

  constructor(
    size: number,
    private readonly feedback: number,
    private readonly damp1: number,
  ) {
    this.buffer = new Float32Array(size);
  }

  process(input: number): number {
    const output = this.buffer[this.index]!;
    this.store = output * (1 - this.damp1) + this.store * this.damp1;
    this.buffer[this.index] = input + this.store * this.feedback;
    this.index += 1;
    if (this.index >= this.buffer.length) {
      this.index = 0;
    }
    return output;
  }
}

class Allpass {
  private readonly buffer: Float32Array;
  private index = 0;

  constructor(size: number) {
    this.buffer = new Float32Array(size);
  }

  process(input: number): number {
    const buffered = this.buffer[this.index]!;
    const output = -input + buffered;
    this.buffer[this.index] = input + buffered * ALLPASS_FEEDBACK;
    this.index += 1;
    if (this.index >= this.buffer.length) {
      this.index = 0;
    }
    return output;
  }
}

class Freeverb implements Reverb {
  private readonly combsL: Comb[];
  private readonly combsR: Comb[];
  private readonly allpassL: Allpass[];
  private readonly allpassR: Allpass[];
  private readonly wet1: number;
  private readonly wet2: number;
  private readonly dry: number;
  private readonly preDelay: Float32Array;
  private readonly preDelayLen: number;
  private preDelayIndex = 0;

  constructor(sampleRate: number, params: ReverbParams) {
    const scale = sampleRate / 44100;
    const feedback = params.roomSize * SCALE_ROOM + OFFSET_ROOM;
    const damp1 = params.damping * SCALE_DAMP;
    this.combsL = COMB_TUNINGS.map((t) => new Comb(Math.round(t * scale), feedback, damp1));
    this.combsR = COMB_TUNINGS.map((t) => new Comb(Math.round((t + STEREO_SPREAD) * scale), feedback, damp1));
    this.allpassL = ALLPASS_TUNINGS.map((t) => new Allpass(Math.round(t * scale)));
    this.allpassR = ALLPASS_TUNINGS.map((t) => new Allpass(Math.round((t + STEREO_SPREAD) * scale)));
    this.wet1 = params.wet * (params.width / 2 + 0.5);
    this.wet2 = params.wet * ((1 - params.width) / 2);
    this.dry = params.dry;
    this.preDelayLen = Math.round((params.preDelayMs / 1000) * sampleRate);
    this.preDelay = new Float32Array(Math.max(1, this.preDelayLen));
  }

  processBlock(left: Float32Array, right: Float32Array): ReverbOutput {
    const n = left.length;
    const outL = new Float32Array(n);
    const outR = new Float32Array(n);
    for (let i = 0; i < n; i += 1) {
      const dryL = left[i]!;
      const dryR = right[i]!;
      let input = (dryL + dryR) * FIXED_GAIN;
      if (this.preDelayLen > 0) {
        const delayed = this.preDelay[this.preDelayIndex]!;
        this.preDelay[this.preDelayIndex] = input;
        this.preDelayIndex += 1;
        if (this.preDelayIndex >= this.preDelayLen) {
          this.preDelayIndex = 0;
        }
        input = delayed;
      }
      let accL = 0;
      let accR = 0;
      for (const comb of this.combsL) {
        accL += comb.process(input);
      }
      for (const comb of this.combsR) {
        accR += comb.process(input);
      }
      for (const allpass of this.allpassL) {
        accL = allpass.process(accL);
      }
      for (const allpass of this.allpassR) {
        accR = allpass.process(accR);
      }
      outL[i] = accL * this.wet1 + accR * this.wet2 + dryL * this.dry;
      outR[i] = accR * this.wet1 + accL * this.wet2 + dryR * this.dry;
    }
    return { left: outL, right: outR };
  }
}

export function createReverb(sampleRate: number, params: ReverbParams): Reverb {
  return new Freeverb(sampleRate, params);
}

const LONGEST_COMB_SECONDS = (COMB_TUNINGS[COMB_TUNINGS.length - 1]! + STEREO_SPREAD) / 44100;
const SILENCE_THRESHOLD = 0.001;

/**
 * Approximate time (seconds) for the reverb tail to decay to roughly -60 dB,
 * derived from the longest comb's delay and its `roomSize`-driven feedback. The
 * mixer uses this to size the render buffer so a long tail is not truncated.
 */
export function reverbDecaySeconds(roomSize: number): number {
  const feedback = roomSize * SCALE_ROOM + OFFSET_ROOM;
  return (LONGEST_COMB_SECONDS * Math.log(SILENCE_THRESHOLD)) / Math.log(feedback);
}
