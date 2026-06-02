import { shapeCurve } from "./curve";
import { lfoValue, type LfoShape } from "./lfo";

/**
 * Time-varying pitch modulation in semitones, combining two independent
 * sources:
 *
 * - a one-shot glide that starts at `glideSemitones` above (or below) the note
 *   and slides to zero over `glideMs`, shaped by `glideCurve` — the building
 *   block for 808-style pitch drops and risers; and
 * - a vibrato LFO of `vibratoCents` depth at `vibratoHz`, held back by
 *   `vibratoDelayMs` and eased in over `vibratoFadeMs`.
 */
export interface PitchModParams {
  glideSemitones: number;
  glideMs: number;
  glideCurve: number;
  vibratoCents: number;
  vibratoHz: number;
  vibratoDelayMs: number;
  vibratoFadeMs: number;
  vibratoShape: LfoShape;
}

function glideOffset(params: PitchModParams, t: number): number {
  if (params.glideMs <= 0) {
    return 0;
  }
  const progress = t / (params.glideMs / 1000);
  return params.glideSemitones * (1 - shapeCurve(progress, params.glideCurve));
}

function vibratoFade(t: number, delaySec: number, fadeSec: number): number {
  if (t <= delaySec) {
    return 0;
  }
  if (fadeSec <= 0) {
    return 1;
  }
  const f = (t - delaySec) / fadeSec;
  return f >= 1 ? 1 : f;
}

function vibratoOffset(params: PitchModParams, t: number): number {
  const depthSemitones = params.vibratoCents / 100;
  const fade = vibratoFade(t, params.vibratoDelayMs / 1000, params.vibratoFadeMs / 1000);
  return depthSemitones * fade * lfoValue(params.vibratoShape, t * params.vibratoHz);
}

export function pitchOffsetSemitones(params: PitchModParams, t: number): number {
  return glideOffset(params, t) + vibratoOffset(params, t);
}
