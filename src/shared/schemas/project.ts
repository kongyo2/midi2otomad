import { z } from "zod";

export const SampleSchema = z.object({
  id: z.string(),
  name: z.string(),
  filePath: z.string(),
  basePitch: z.number().int().min(0).max(127),
  gain: z.number().min(0).max(4).default(1),
});

export const NoteSchema = z.object({
  pitch: z.number().int().min(0).max(127),
  startTick: z.number().int().min(0),
  durationTicks: z.number().int().positive(),
  velocity: z.number().int().min(0).max(127).default(100),
});

export const TrackSchema = z.object({
  id: z.string(),
  name: z.string(),
  sampleId: z.string(),
  muted: z.boolean().default(false),
  notes: z.array(NoteSchema),
});

export const ProjectSchema = z.object({
  version: z.literal(1),
  name: z.string().min(1),
  bpm: z.number().positive(),
  ppq: z.number().int().positive().default(480),
  samples: z.array(SampleSchema),
  tracks: z.array(TrackSchema),
});

export type Sample = z.infer<typeof SampleSchema>;
export type Note = z.infer<typeof NoteSchema>;
export type Track = z.infer<typeof TrackSchema>;
export type Project = z.infer<typeof ProjectSchema>;

export function parseProject(raw: unknown): Project {
  return ProjectSchema.parse(raw);
}
