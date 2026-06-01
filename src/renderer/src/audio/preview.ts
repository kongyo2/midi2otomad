import { parseProject, type Sample } from "../../../shared/schemas/project";
import { bankFromRecord, mixProject, type PcmAudio } from "../../../shared/audio/mixer";
import { getAudioContext, resumeAudioContext } from "./context";

/**
 * Audition a single material one-shot by rendering it through the very same
 * offline engine used for playback and export. The envelope, dynamic pitch and
 * filter — including their envelope/LFO modulation — therefore sound exactly as
 * they will in the rendered mix, with no preview-only signal path to drift out
 * of sync.
 */
export function previewSample(pcm: PcmAudio, sample: Sample, pitch?: number): void {
  const ctx = getAudioContext();
  void resumeAudioContext();

  const durationSec = Math.max(0.05, sample.loop.enabled ? 1.4 : Math.min(2.2, pcm.frames / pcm.sampleRate));
  const project = parseProject({
    version: 1,
    name: "preview",
    sampleRate: ctx.sampleRate,
    samples: [sample],
    tracks: [
      {
        id: "preview",
        name: "preview",
        defaultSampleId: sample.id,
        notes: [{ pitch: pitch ?? sample.basePitch, startSec: 0, durationSec, velocity: 127 }],
      },
    ],
  });

  const mix = mixProject(project, bankFromRecord({ [sample.id]: pcm }), { limiter: false, tailSec: 0.1 });
  const buffer = ctx.createBuffer(2, mix.frames, mix.sampleRate);
  buffer.copyToChannel(mix.left as Float32Array<ArrayBuffer>, 0);
  buffer.copyToChannel(mix.right as Float32Array<ArrayBuffer>, 1);

  const source = ctx.createBufferSource();
  source.buffer = buffer;
  source.connect(ctx.destination);
  source.start();
}
