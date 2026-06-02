import { shapeCurve } from "./curve";

/**
 * A delay / attack / hold / decay / sustain / release envelope with an
 * independent curve shape on each sloped segment. This is a strict superset of
 * a plain attack/release pair: leave the extra stages at their defaults
 * (zero-length delay/hold/decay and a sustain of 1) and only the attack ramp
 * and release tail remain.
 */
export interface EnvelopeParams {
  delayMs: number;
  attackMs: number;
  attackCurve: number;
  holdMs: number;
  decayMs: number;
  decayCurve: number;
  sustain: number;
  releaseMs: number;
  releaseCurve: number;
}

/** Level reached while the note is still held, ignoring the release tail. */
function preReleaseLevel(env: EnvelopeParams, t: number): number {
  const delay = env.delayMs / 1000;
  const attack = env.attackMs / 1000;
  const hold = env.holdMs / 1000;
  const decay = env.decayMs / 1000;
  const attackEnd = delay + attack;
  const holdEnd = attackEnd + hold;
  const decayEnd = holdEnd + decay;
  if (t < delay) {
    return 0;
  }
  if (t < attackEnd) {
    return shapeCurve((t - delay) / attack, env.attackCurve);
  }
  if (t < holdEnd) {
    return 1;
  }
  if (t < decayEnd) {
    return 1 - (1 - env.sustain) * shapeCurve((t - holdEnd) / decay, env.decayCurve);
  }
  return env.sustain;
}

/**
 * Envelope gain in `[0, 1]` at time `t` (seconds since note-on) for a note held
 * open until `gateSec`. After the gate closes the envelope releases from
 * whatever level it had reached toward zero over the release time.
 */
export function envelopeLevel(env: EnvelopeParams, t: number, gateSec: number): number {
  if (t < gateSec) {
    return preReleaseLevel(env, t);
  }
  const release = env.releaseMs / 1000;
  if (release <= 0) {
    return 0;
  }
  const r = (t - gateSec) / release;
  if (r >= 1) {
    return 0;
  }
  return preReleaseLevel(env, gateSec) * (1 - shapeCurve(r, env.releaseCurve));
}
