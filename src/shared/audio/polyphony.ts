import type { Polyphony, StopMode, VoicePriority } from "../schemas/project";

/** A single note competing for a voice on a track. */
export interface VoiceRequest {
  pitch: number;
  startSec: number;
  durationSec: number;
  sampleId: string;
}

/** A surviving voice: which request it came from and how long its gate stays open. */
export interface VoiceAllocation {
  index: number;
  durationSec: number;
}

interface ActiveVoice {
  index: number;
  pitch: number;
  startSec: number;
  sampleId: string;
  endSec: number;
}

/**
 * Orders the held voices least-valuable first, so the front of the order is the
 * one to sacrifice when a fresh note overflows the cap.
 */
const VICTIM_ORDER: Record<VoicePriority, (a: ActiveVoice, b: ActiveVoice) => number> = {
  newest: (a, b) => a.startSec - b.startSec || a.index - b.index,
  oldest: (a, b) => b.startSec - a.startSec || b.index - a.index,
  highest: (a, b) => a.pitch - b.pitch || a.index - b.index,
  lowest: (a, b) => b.pitch - a.pitch || a.index - b.index,
};

/** Whether a held voice belongs to the same choke group as an incoming note. */
const SHARES_GROUP: Record<Exclude<StopMode, "none">, (held: ActiveVoice, incoming: VoiceRequest) => boolean> = {
  pitch: (held, incoming) => held.pitch === incoming.pitch,
  sample: (held, incoming) => held.sampleId === incoming.sampleId,
  track: () => true,
};

export function allocateVoices(requests: VoiceRequest[], config: Polyphony): VoiceAllocation[] {
  const cap = config.maxVoices > 0 ? config.maxVoices : Number.POSITIVE_INFINITY;
  const events = requests
    .map((request, index) => ({ ...request, index }))
    .sort((a, b) => a.startSec - b.startSec || a.index - b.index);

  const active: ActiveVoice[] = [];
  const durations = new Map<number, number>();

  for (const event of events) {
    const t = event.startSec;
    for (let i = active.length - 1; i >= 0; i -= 1) {
      if (active[i]!.endSec <= t) {
        active.splice(i, 1);
      }
    }

    if (config.stopMode !== "none") {
      const shares = SHARES_GROUP[config.stopMode];
      for (let i = active.length - 1; i >= 0; i -= 1) {
        const held = active[i]!;
        if (held.startSec < t && shares(held, event)) {
          durations.set(held.index, t - held.startSec);
          active.splice(i, 1);
        }
      }
    }

    active.push({
      index: event.index,
      pitch: event.pitch,
      startSec: event.startSec,
      sampleId: event.sampleId,
      endSec: event.startSec + event.durationSec,
    });
    durations.set(event.index, event.durationSec);

    if (active.length > cap) {
      const lessValuable = VICTIM_ORDER[config.priority];
      let pick = 0;
      for (let i = 1; i < active.length; i += 1) {
        if (lessValuable(active[i]!, active[pick]!) < 0) {
          pick = i;
        }
      }
      const victim = active.splice(pick, 1)[0]!;
      if (victim.startSec < t) {
        durations.set(victim.index, t - victim.startSec);
      } else {
        durations.delete(victim.index);
      }
    }
  }

  return [...durations.entries()]
    .map(([index, durationSec]) => ({ index, durationSec }))
    .sort((a, b) => a.index - b.index);
}
