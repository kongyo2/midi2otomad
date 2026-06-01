import { z } from "zod";

export const DEFAULT_BASE_PITCH = 60;
export const DEFAULT_SAMPLE_RATE = 48000;

export const EnvelopeSchema = z.object({
  attackMs: z.number().min(0).max(5000).default(4),
  releaseMs: z.number().min(0).max(20000).default(90),
});

export const LoopSchema = z.object({
  enabled: z.boolean().default(false),
  startSec: z.number().min(0).default(0),
  endSec: z.number().min(0).default(0),
});

export const SampleSchema = z.object({
  id: z.string(),
  name: z.string(),
  fileName: z.string().default(""),
  basePitch: z.number().int().min(0).max(127).default(DEFAULT_BASE_PITCH),
  tuneCents: z.number().min(-2400).max(2400).default(0),
  gain: z.number().min(0).max(4).default(1),
  durationSec: z.number().min(0).default(0),
  loop: LoopSchema.default({ enabled: false, startSec: 0, endSec: 0 }),
  envelope: EnvelopeSchema.default({ attackMs: 4, releaseMs: 90 }),
});

export const NoteSchema = z.object({
  pitch: z.number().int().min(0).max(127),
  startSec: z.number().min(0),
  durationSec: z.number().positive(),
  velocity: z.number().int().min(0).max(127).default(100),
});

export const AutomationPointSchema = z.object({
  t: z.number().min(0),
  v: z.number().min(0).max(1),
});

export const TrackDynamicsSchema = z.object({
  volume: z.array(AutomationPointSchema).default([]),
  expression: z.array(AutomationPointSchema).default([]),
});

export const TrackSchema = z.object({
  id: z.string(),
  name: z.string(),
  midiIndex: z.number().int().min(0).optional(),
  color: z.string().default("#7c5cff"),
  muted: z.boolean().default(false),
  solo: z.boolean().default(false),
  gain: z.number().min(0).max(4).default(1),
  pan: z.number().min(-1).max(1).default(0),
  defaultSampleId: z.string().nullable().default(null),
  noteSampleMap: z.record(z.string(), z.string()).default({}),
  notes: z.array(NoteSchema).default([]),
  dynamics: TrackDynamicsSchema.default({ volume: [], expression: [] }),
});

export const TempoSchema = z.object({
  timeSec: z.number().min(0),
  bpm: z.number().positive(),
});

export const ProjectSchema = z.object({
  version: z.literal(1),
  name: z.string().min(1),
  bpm: z.number().positive().default(140),
  ppq: z.number().int().positive().default(480),
  sampleRate: z.number().int().positive().default(DEFAULT_SAMPLE_RATE),
  masterGain: z.number().min(0).max(4).default(1),
  tempos: z.array(TempoSchema).default([]),
  samples: z.array(SampleSchema).default([]),
  tracks: z.array(TrackSchema).default([]),
});

export type Envelope = z.infer<typeof EnvelopeSchema>;
export type Loop = z.infer<typeof LoopSchema>;
export type Sample = z.infer<typeof SampleSchema>;
export type Note = z.infer<typeof NoteSchema>;
export type AutomationPoint = z.infer<typeof AutomationPointSchema>;
export type TrackDynamics = z.infer<typeof TrackDynamicsSchema>;
export type Track = z.infer<typeof TrackSchema>;
export type Tempo = z.infer<typeof TempoSchema>;
export type Project = z.infer<typeof ProjectSchema>;

export function parseProject(raw: unknown): Project {
  return ProjectSchema.parse(raw);
}

export function createEmptyProject(name = "Untitled 音MAD"): Project {
  return ProjectSchema.parse({
    version: 1,
    name,
    bpm: 140,
    ppq: 480,
    sampleRate: DEFAULT_SAMPLE_RATE,
    masterGain: 1,
    tempos: [],
    samples: [],
    tracks: [],
  });
}
