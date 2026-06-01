import { DEFAULT_SAMPLE_RATE } from "../../../shared/schemas/project";

let context: AudioContext | null = null;

export function getAudioContext(): AudioContext {
  if (context === null) {
    try {
      context = new AudioContext({ sampleRate: DEFAULT_SAMPLE_RATE });
    } catch {
      context = new AudioContext();
    }
  }
  return context;
}

export async function resumeAudioContext(): Promise<void> {
  const ctx = getAudioContext();
  if (ctx.state === "suspended") {
    await ctx.resume();
  }
}
