import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, renderHook, screen } from "@testing-library/react";
import { parseProject, createEmptyProject } from "../../../shared/schemas/project";
import type { PcmAudio } from "../../../shared/audio/mixer";

const mocks = vi.hoisted(() => {
  const engineInstances: Array<Record<string, unknown>> = [];
  class FakePreviewEngine {
    onEnded: (() => void) | null = null;
    transport = "stopped";
    setMix = vi.fn();
    play = vi.fn(() => {
      this.transport = "playing";
    });
    pause = vi.fn(() => {
      this.transport = "paused";
    });
    stop = vi.fn(() => {
      this.transport = "stopped";
    });
    seek = vi.fn();
    getPosition = vi.fn(() => 5);
    getMasterAnalyser = vi.fn(() => ({}));
    constructor() {
      engineInstances.push(this as unknown as Record<string, unknown>);
    }
  }
  return {
    engineInstances,
    FakePreviewEngine,
    decodeAudio: vi.fn(),
    buildWaveformPeaks: vi.fn(() => new Float32Array([0.5])),
    midiToProject: vi.fn(),
    mixProject: vi.fn(),
  };
});

vi.mock("../audio/engine", () => ({ PreviewEngine: mocks.FakePreviewEngine }));
vi.mock("../audio/decode", () => ({ decodeAudio: mocks.decodeAudio, buildWaveformPeaks: mocks.buildWaveformPeaks }));
vi.mock("../midi/import", () => ({ midiToProject: mocks.midiToProject }));
vi.mock("../../../shared/audio/mixer", () => ({ mixProject: mocks.mixProject }));

import { StudioProvider, projectReducer, useStudio, usePlayhead } from "./StudioContext";

const pcm: PcmAudio = { sampleRate: 48000, channels: [new Float32Array(10)], frames: 10 };

function renderStudio() {
  return renderHook(() => useStudio(), { wrapper: StudioProvider });
}

beforeEach(() => {
  mocks.engineInstances.length = 0;
  mocks.decodeAudio.mockResolvedValue(pcm);
  mocks.midiToProject.mockReset();
  mocks.mixProject.mockImplementation((_project: unknown, bank: { get: (id: string) => unknown }) => {
    bank.get("probe");
    return {
      sampleRate: 48000,
      left: new Float32Array(4),
      right: new Float32Array(4),
      frames: 4,
      durationSec: 1,
      peak: 0.5,
    };
  });
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

describe("projectReducer", () => {
  const base = parseProject({
    version: 1,
    name: "p",
    samples: [
      { id: "s1", name: "a" },
      { id: "s2", name: "b" },
    ],
    tracks: [
      { id: "t1", name: "A", defaultSampleId: null, noteSampleMap: { "60": "s1", "64": "s2" }, notes: [] },
      { id: "t2", name: "B", defaultSampleId: "s1", noteSampleMap: {}, notes: [] },
    ],
  });
  const sample = {
    id: "s3",
    name: "c",
    fileName: "",
    basePitch: 60,
    tuneCents: 0,
    gain: 1,
    durationSec: 1,
    loop: { enabled: false, startSec: 0, endSec: 0 },
    envelope: { attackMs: 4, releaseMs: 90 },
  };

  it("replaces the project", () => {
    const next = createEmptyProject("x");
    expect(projectReducer(base, { type: "setProject", project: next })).toBe(next);
  });

  it("patches top-level fields", () => {
    expect(projectReducer(base, { type: "patchProject", patch: { bpm: 99 } }).bpm).toBe(99);
  });

  it("patches the master reverb bus", () => {
    const next = projectReducer(base, {
      type: "patchProject",
      patch: { reverb: { ...base.reverb, enabled: true, roomSize: 0.9 } },
    });
    expect(next.reverb.enabled).toBe(true);
    expect(next.reverb.roomSize).toBe(0.9);
  });

  it("adds a sample and optionally assigns it to unassigned tracks", () => {
    const assigned = projectReducer(base, { type: "addSample", sample, assignToTracks: true });
    expect(assigned.samples).toHaveLength(3);
    expect(assigned.tracks[0]!.defaultSampleId).toBe("s3");
    expect(assigned.tracks[1]!.defaultSampleId).toBe("s1");

    const unassigned = projectReducer(base, { type: "addSample", sample, assignToTracks: false });
    expect(unassigned.tracks[0]!.defaultSampleId).toBeNull();
  });

  it("updates a sample only when the id matches", () => {
    const next = projectReducer(base, { type: "updateSample", id: "s1", patch: { name: "renamed" } });
    expect(next.samples[0]!.name).toBe("renamed");
    expect(next.samples[1]!.name).toBe("b");
  });

  it("removes a sample and detaches it everywhere", () => {
    const next = projectReducer(base, { type: "removeSample", id: "s1" });
    expect(next.samples.map((s) => s.id)).toEqual(["s2"]);
    expect(next.tracks[0]!.noteSampleMap).toEqual({ "64": "s2" });
    expect(next.tracks[0]!.defaultSampleId).toBeNull();
    expect(next.tracks[1]!.defaultSampleId).toBeNull();
  });

  it("updates a track only when the id matches", () => {
    const next = projectReducer(base, { type: "updateTrack", id: "t2", patch: { name: "Bee" } });
    expect(next.tracks[1]!.name).toBe("Bee");
    expect(next.tracks[0]!.name).toBe("A");
  });

  it("sets and clears per-note sample overrides", () => {
    const set = projectReducer(base, { type: "setNoteSample", trackId: "t1", note: 67, sampleId: "s2" });
    expect(set.tracks[0]!.noteSampleMap["67"]).toBe("s2");
    const cleared = projectReducer(base, { type: "setNoteSample", trackId: "t1", note: 60, sampleId: null });
    expect(cleared.tracks[0]!.noteSampleMap["60"]).toBeUndefined();
    const untouched = projectReducer(base, { type: "setNoteSample", trackId: "zzz", note: 60, sampleId: "s2" });
    expect(untouched.tracks[0]!.noteSampleMap).toEqual(base.tracks[0]!.noteSampleMap);
  });

  it("returns the project unchanged for an unknown action", () => {
    expect(projectReducer(base, { type: "nope" } as never)).toBe(base);
  });
});

describe("StudioProvider state", () => {
  it("starts from an empty project", () => {
    const { result } = renderStudio();
    expect(result.current.project.name).toBe("Untitled 音MAD");
    expect(result.current.isPlaying).toBe(false);
  });

  it("tracks selections", () => {
    const { result } = renderStudio();
    act(() => {
      result.current.selectTrack("t1");
      result.current.selectSample("s1");
    });
    expect(result.current.selectedTrackId).toBe("t1");
    expect(result.current.selectedSampleId).toBe("s1");
  });

  it("imports MIDI and selects the first track", () => {
    const project = parseProject({
      version: 1,
      name: "imported",
      tracks: [{ id: "ta", name: "lead", notes: [{ pitch: 60, startSec: 0, durationSec: 1 }] }],
    });
    mocks.midiToProject.mockReturnValue({ project, trackCount: 1, noteCount: 1 });
    const { result } = renderStudio();
    act(() => {
      result.current.importMidiBytes(new Uint8Array([1]), "song.mid");
    });
    expect(result.current.project.name).toBe("imported");
    expect(result.current.selectedTrackId).toBe("ta");
    expect(result.current.toast).toContain("song.mid");
  });

  it("imports MIDI with no tracks and clears the selection", () => {
    mocks.midiToProject.mockReturnValue({ project: createEmptyProject("blank"), trackCount: 0, noteCount: 0 });
    const { result } = renderStudio();
    act(() => {
      result.current.selectTrack("old");
      result.current.importMidiBytes(new Uint8Array([1]), "blank.mid");
    });
    expect(result.current.selectedTrackId).toBeNull();
  });

  it("reports MIDI import failures", () => {
    mocks.midiToProject.mockImplementation(() => {
      throw new Error("bad midi");
    });
    const { result } = renderStudio();
    act(() => {
      result.current.importMidiBytes(new Uint8Array([1]), "x.mid");
    });
    expect(result.current.toast).toContain("bad midi");

    mocks.midiToProject.mockImplementation(() => {
      throw "weird";
    });
    act(() => {
      result.current.importMidiBytes(new Uint8Array([1]), "y.mid");
    });
    expect(result.current.toast).toContain("weird");
  });

  it("ingests audio, assigns the first sample, and stores buffers", async () => {
    const { result } = renderStudio();
    await act(async () => {
      await result.current.ingestAudio([new File([new Uint8Array([1])], "kick.wav")]);
    });
    const sampleId = result.current.selectedSampleId!;
    expect(sampleId).not.toBeNull();
    expect(result.current.project.samples).toHaveLength(1);
    expect(result.current.getAudio(sampleId)).toBe(pcm);
    expect(result.current.getPeaks(sampleId)).toBeInstanceOf(Float32Array);
    expect(result.current.getAudio("missing")).toBeUndefined();
    expect(result.current.busy).toBeNull();
  });

  it("ingests a pre-loaded file payload", async () => {
    const { result } = renderStudio();
    await act(async () => {
      await result.current.ingestAudio([{ name: "loaded.wav", data: new Uint8Array([9, 9]) }]);
    });
    expect(result.current.project.samples).toHaveLength(1);
    expect(result.current.project.samples[0]!.fileName).toBe("loaded.wav");
  });

  it("ingests audio onto a specific track when requested", async () => {
    const project = parseProject({
      version: 1,
      name: "p",
      samples: [{ id: "existing", name: "old" }],
      tracks: [{ id: "t1", name: "A", notes: [] }],
    });
    mocks.midiToProject.mockReturnValue({ project, trackCount: 1, noteCount: 0 });
    const { result } = renderStudio();
    act(() => {
      result.current.importMidiBytes(new Uint8Array([1]), "p.mid");
    });
    await act(async () => {
      await result.current.ingestAudio([new File([new Uint8Array([1])], "snare.wav")], "t1");
    });
    const track = result.current.project.tracks.find((t) => t.id === "t1")!;
    expect(track.defaultSampleId).not.toBeNull();
  });

  it("ingests several files at once", async () => {
    const { result } = renderStudio();
    await act(async () => {
      await result.current.ingestAudio([
        new File([new Uint8Array([1])], "a.wav"),
        new File([new Uint8Array([2])], "b.wav"),
      ]);
    });
    expect(result.current.project.samples).toHaveLength(2);
    expect(result.current.toast).toContain("2 個");
  });

  it("does nothing when handed an empty file list for a track", async () => {
    const { result } = renderStudio();
    await act(async () => {
      await result.current.ingestAudio([], "t1");
    });
    expect(result.current.project.samples).toHaveLength(0);
    expect(result.current.selectedSampleId).toBeNull();
    expect(result.current.busy).toBeNull();
  });

  it("reports audio decode failures, including non-Error throws", async () => {
    mocks.decodeAudio.mockRejectedValueOnce(new Error("decode failed"));
    const { result } = renderStudio();
    await act(async () => {
      await result.current.ingestAudio([new File([new Uint8Array([1])], "bad.wav")]);
    });
    expect(result.current.toast).toContain("decode failed");
    expect(result.current.busy).toBeNull();

    mocks.decodeAudio.mockRejectedValueOnce("kaboom");
    await act(async () => {
      await result.current.ingestAudio([new File([new Uint8Array([1])], "bad.wav")]);
    });
    expect(result.current.toast).toContain("kaboom");
  });

  it("edits samples, tracks and project fields", () => {
    const { result } = renderStudio();
    act(() => {
      result.current.patchProject({ name: "renamed" });
      result.current.patchProject({ bpm: 90 });
      result.current.updateTrack("t1", { gain: 0.5 });
      result.current.updateSample("s1", { gain: 2 });
      result.current.setNoteSample("t1", 60, "s1");
    });
    expect(result.current.project.name).toBe("renamed");
    expect(result.current.project.bpm).toBe(90);
  });

  it("re-renders the mix when master gain or the reverb bus change", () => {
    const { result } = renderStudio();
    act(() => {
      result.current.play();
    });
    expect(mocks.mixProject).toHaveBeenCalledTimes(1);

    act(() => {
      result.current.patchProject({ masterGain: 0.5 });
    });
    act(() => {
      result.current.play();
    });
    expect(mocks.mixProject).toHaveBeenCalledTimes(2);

    act(() => {
      result.current.patchProject({ reverb: { ...result.current.project.reverb, enabled: true } });
    });
    act(() => {
      result.current.play();
    });
    expect(mocks.mixProject).toHaveBeenCalledTimes(3);
  });

  it("clears the selected sample when it is removed", async () => {
    const { result } = renderStudio();
    await act(async () => {
      await result.current.ingestAudio([new File([new Uint8Array([1])], "kick.wav")]);
    });
    const sampleId = result.current.selectedSampleId!;
    act(() => {
      result.current.removeSample(sampleId);
    });
    expect(result.current.selectedSampleId).toBeNull();
    expect(result.current.getAudio(sampleId)).toBeUndefined();

    act(() => {
      result.current.selectSample("kept");
      result.current.removeSample("other");
    });
    expect(result.current.selectedSampleId).toBe("kept");
  });

  it("plays, re-uses the mix, toggles, pauses, stops and seeks", () => {
    const { result } = renderStudio();
    act(() => {
      result.current.play();
    });
    expect(result.current.isPlaying).toBe(true);
    expect(mocks.mixProject).toHaveBeenCalledTimes(1);
    const engine = mocks.engineInstances[0]!;
    expect(engine.play).toHaveBeenCalled();

    act(() => {
      result.current.play();
    });
    expect(mocks.mixProject).toHaveBeenCalledTimes(1);

    act(() => {
      result.current.togglePlay();
    });
    expect(result.current.isPlaying).toBe(false);
    expect(engine.pause).toHaveBeenCalled();

    act(() => {
      result.current.togglePlay();
    });
    expect(result.current.isPlaying).toBe(true);

    act(() => {
      result.current.stop();
    });
    expect(engine.stop).toHaveBeenCalled();

    act(() => {
      result.current.seek(2);
    });
    expect(engine.seek).toHaveBeenCalledWith(2);
  });

  it("notifies when playback ends", () => {
    const { result } = renderStudio();
    act(() => {
      result.current.play();
    });
    const engine = mocks.engineInstances[0]!;
    act(() => {
      (engine.onEnded as () => void)();
    });
    expect(result.current.isPlaying).toBe(false);
  });

  it("ignores pause and stop before an engine exists", () => {
    const { result } = renderStudio();
    act(() => {
      result.current.pause();
      result.current.stop();
      result.current.togglePlay();
    });
    expect(result.current.isPlaying).toBe(true);
    expect(mocks.engineInstances).toHaveLength(1);
  });

  it("warns when there is nothing to export", async () => {
    mocks.mixProject.mockReturnValue({
      sampleRate: 48000,
      left: new Float32Array(1),
      right: new Float32Array(1),
      frames: 1,
      durationSec: 0,
      peak: 0,
    });
    const bounce = vi.fn();
    vi.stubGlobal("api", { bounce });
    const { result } = renderStudio();
    await act(async () => {
      await result.current.exportMix({ format: "wav" });
    });
    expect(result.current.toast).toContain("書き出す音がありません");
    expect(bounce).not.toHaveBeenCalled();
    expect(result.current.busy).toBeNull();
  });

  it("exports successfully and reports the path", async () => {
    const bounce = vi.fn(async () => ({ ok: true, path: "/out.wav", bytes: 4096, durationSec: 2 }));
    vi.stubGlobal("api", { bounce });
    const { result } = renderStudio();
    await act(async () => {
      await result.current.exportMix({ format: "wav", wavBitDepth: 24 });
    });
    expect(bounce).toHaveBeenCalledWith(expect.objectContaining({ format: "wav", wavBitDepth: 24 }));
    expect(result.current.toast).toContain("/out.wav");
  });

  it("reports a canceled export", async () => {
    vi.stubGlobal("api", { bounce: vi.fn(async () => ({ ok: false, canceled: true })) });
    const { result } = renderStudio();
    await act(async () => {
      await result.current.exportMix({ format: "mp3", mp3Bitrate: 256 });
    });
    expect(result.current.toast).toContain("キャンセル");
  });

  it("reports an export error, defaulting the message", async () => {
    vi.stubGlobal("api", { bounce: vi.fn(async () => ({ ok: false, canceled: false })) });
    const { result } = renderStudio();
    await act(async () => {
      await result.current.exportMix({ format: "mp3" });
    });
    expect(result.current.toast).toContain("unknown");
  });

  it("surfaces an explicit export error and thrown failures", async () => {
    vi.stubGlobal("api", { bounce: vi.fn(async () => ({ ok: false, canceled: false, error: "disk full" })) });
    const { result } = renderStudio();
    await act(async () => {
      await result.current.exportMix({ format: "wav" });
    });
    expect(result.current.toast).toContain("disk full");

    vi.stubGlobal("api", {
      bounce: vi.fn(async () => {
        throw new Error("crash");
      }),
    });
    await act(async () => {
      await result.current.exportMix({ format: "wav" });
    });
    expect(result.current.toast).toContain("crash");

    vi.stubGlobal("api", {
      bounce: vi.fn(async () => {
        throw "stringy";
      }),
    });
    await act(async () => {
      await result.current.exportMix({ format: "wav" });
    });
    expect(result.current.toast).toContain("stringy");
  });

  it("auto-dismisses toasts and replaces an active one", () => {
    vi.useFakeTimers();
    try {
      const { result } = renderStudio();
      act(() => {
        result.current.showToast("first");
      });
      expect(result.current.toast).toBe("first");
      act(() => {
        result.current.showToast("second");
      });
      expect(result.current.toast).toBe("second");
      act(() => {
        vi.advanceTimersByTime(3200);
      });
      expect(result.current.toast).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });
});

describe("useStudio", () => {
  it("throws when used outside of a provider", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => undefined);
    expect(() => renderHook(() => useStudio())).toThrow(/within a StudioProvider/);
    spy.mockRestore();
  });
});

describe("usePlayhead", () => {
  it("polls the engine position once it exists", () => {
    function Probe(): React.JSX.Element {
      const studio = useStudio();
      const position = usePlayhead();
      return (
        <button type="button" onClick={() => studio.play()}>
          {position}
        </button>
      );
    }
    const rafSpy = vi.spyOn(window, "requestAnimationFrame");
    render(
      <StudioProvider>
        <Probe />
      </StudioProvider>,
    );
    const tick = rafSpy.mock.calls[0]![0];
    act(() => {
      tick(0);
    });
    expect(screen.getByRole("button")).toHaveTextContent("0");

    act(() => {
      screen.getByRole("button").click();
    });
    act(() => {
      tick(0);
    });
    expect(screen.getByRole("button")).toHaveTextContent("5");
  });
});
