import { z } from "zod";

export const DEFAULT_BASE_PITCH = 60;
export const DEFAULT_SAMPLE_RATE = 48000;

export const FILTER_TYPES = [
  "lowpass",
  "highpass",
  "bandpass",
  "notch",
  "peaking",
  "lowshelf",
  "highshelf",
  "allpass",
] as const;

export const LFO_SHAPES = ["sine", "triangle", "square", "saw"] as const;

export const INTERPOLATION_MODES = ["linear", "hermite"] as const;

const CURVE = z.number().min(-8).max(8).default(0);

export const EnvelopeSchema = z.object({
  delayMs: z.number().min(0).max(5000).default(0),
  attackMs: z.number().min(0).max(5000).default(4),
  attackCurve: CURVE,
  holdMs: z.number().min(0).max(5000).default(0),
  decayMs: z.number().min(0).max(20000).default(0),
  decayCurve: CURVE,
  sustain: z.number().min(0).max(1).default(1),
  releaseMs: z.number().min(0).max(20000).default(90),
  releaseCurve: CURVE,
});

export const FilterSchema = z.object({
  enabled: z.boolean().default(false),
  type: z.enum(FILTER_TYPES).default("lowpass"),
  cutoffHz: z.number().min(20).max(20000).default(20000),
  q: z.number().min(0.1).max(24).default(0.707),
  gainDb: z.number().min(-24).max(24).default(0),
});

export const PitchModSchema = z.object({
  glideSemitones: z.number().min(-48).max(48).default(0),
  glideMs: z.number().min(0).max(5000).default(0),
  glideCurve: CURVE,
  vibratoCents: z.number().min(0).max(1200).default(0),
  vibratoHz: z.number().min(0).max(20).default(5),
  vibratoDelayMs: z.number().min(0).max(5000).default(0),
  vibratoFadeMs: z.number().min(0).max(5000).default(0),
  vibratoShape: z.enum(LFO_SHAPES).default("sine"),
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
  interpolation: z.enum(INTERPOLATION_MODES).default("hermite"),
  loop: LoopSchema.default({ enabled: false, startSec: 0, endSec: 0 }),
  envelope: EnvelopeSchema.prefault({}),
  filter: FilterSchema.prefault({}),
  pitchMod: PitchModSchema.prefault({}),
});

export const ReverbSchema = z.object({
  enabled: z.boolean().default(false),
  roomSize: z.number().min(0).max(1).default(0.5),
  damping: z.number().min(0).max(1).default(0.5),
  width: z.number().min(0).max(1).default(1),
  wet: z.number().min(0).max(1).default(0.25),
  dry: z.number().min(0).max(1).default(1),
  preDelayMs: z.number().min(0).max(500).default(0),
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
  reverbSend: z.number().min(0).max(1).default(0),
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
  reverb: ReverbSchema.prefault({}),
});

export type Envelope = z.infer<typeof EnvelopeSchema>;
export type Filter = z.infer<typeof FilterSchema>;
export type FilterType = (typeof FILTER_TYPES)[number];
export type LfoShape = (typeof LFO_SHAPES)[number];
export type InterpolationMode = (typeof INTERPOLATION_MODES)[number];
export type PitchMod = z.infer<typeof PitchModSchema>;
export type Reverb = z.infer<typeof ReverbSchema>;
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

export function createSample(input: { id: string; name: string } & Partial<Sample>): Sample {
  return SampleSchema.parse(input);
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
