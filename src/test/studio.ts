import { vi } from "vitest";
import type { StudioContextValue } from "../renderer/src/state/StudioContext";
import { createEmptyProject } from "../shared/schemas/project";

/** A fully-populated StudioContextValue with spies, for isolating component tests. */
export function makeStudioValue(overrides: Partial<StudioContextValue> = {}): StudioContextValue {
  return {
    project: createEmptyProject(),
    selectedTrackId: null,
    selectedSampleId: null,
    isPlaying: false,
    busy: null,
    toast: null,
    engineRef: { current: null },
    selectTrack: vi.fn(),
    selectSample: vi.fn(),
    importMidiBytes: vi.fn(),
    ingestAudio: vi.fn(async () => undefined),
    updateSample: vi.fn(),
    removeSample: vi.fn(),
    patchProject: vi.fn(),
    updateTrack: vi.fn(),
    setNoteSample: vi.fn(),
    getAudio: vi.fn(() => undefined),
    getPeaks: vi.fn(() => undefined),
    play: vi.fn(),
    pause: vi.fn(),
    stop: vi.fn(),
    togglePlay: vi.fn(),
    seek: vi.fn(),
    exportMix: vi.fn(async () => undefined),
    showToast: vi.fn(),
    ...overrides,
  };
}
