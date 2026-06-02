import { describe, expect, it, vi, beforeEach } from "vitest";
import { fireEvent, render, screen, within } from "@testing-library/react";
import type { StudioContextValue } from "../state/StudioContext";
import { makeStudioValue } from "../../../test/studio";
import { parseProject } from "../../../shared/schemas/project";

const holder = vi.hoisted(() => ({ value: null as StudioContextValue | null }));

vi.mock("../state/StudioContext", () => ({
  useStudio: () => holder.value,
}));

import { TrackInspector } from "./TrackInspector";

function projectWithTrack(over: Record<string, unknown> = {}): ReturnType<typeof parseProject> {
  return parseProject({
    version: 1,
    name: "p",
    samples: [
      { id: "s1", name: "Kick" },
      { id: "s2", name: "Snare" },
    ],
    tracks: [
      {
        id: "t1",
        name: "Lead",
        color: "#36d399",
        gain: 1,
        pan: 0,
        defaultSampleId: "s1",
        noteSampleMap: { "67": "s2" },
        notes: [
          { pitch: 60, startSec: 0, durationSec: 1, velocity: 100 },
          { pitch: 67, startSec: 1, durationSec: 1, velocity: 100 },
          { pitch: 60, startSec: 2, durationSec: 1, velocity: 100 },
        ],
        dynamics: { volume: [], expression: [] },
        ...over,
      },
    ],
  });
}

beforeEach(() => {
  holder.value = makeStudioValue();
});

describe("TrackInspector", () => {
  it("shows a placeholder when no track is selected", () => {
    holder.value = makeStudioValue({ project: projectWithTrack(), selectedTrackId: null });
    render(<TrackInspector />);
    expect(screen.getByText(/音量・パン・素材割り当てを編集/)).toBeInTheDocument();
  });

  it("edits name, default sample, gain and pan", () => {
    const value = makeStudioValue({ project: projectWithTrack(), selectedTrackId: "t1" });
    holder.value = value;
    render(<TrackInspector />);

    expect(screen.getByText("3 ノート")).toBeInTheDocument();

    fireEvent.change(screen.getByDisplayValue("Lead"), { target: { value: "Bass" } });
    expect(value.updateTrack).toHaveBeenCalledWith("t1", { name: "Bass" });

    const sampleSelect = screen.getByRole("combobox", { name: /既定の音声素材/ });
    fireEvent.change(sampleSelect, { target: { value: "s2" } });
    expect(value.updateTrack).toHaveBeenCalledWith("t1", { defaultSampleId: "s2" });
    fireEvent.change(sampleSelect, { target: { value: "" } });
    expect(value.updateTrack).toHaveBeenCalledWith("t1", { defaultSampleId: null });

    const ranges = screen.getAllByRole("slider");
    fireEvent.change(ranges[0]!, { target: { value: "2" } });
    expect(value.updateTrack).toHaveBeenCalledWith("t1", { gain: 2 });
    fireEvent.change(ranges[1]!, { target: { value: "0.5" } });
    expect(value.updateTrack).toHaveBeenCalledWith("t1", { pan: 0.5 });
  });

  it("edits the reverb send amount", () => {
    const value = makeStudioValue({ project: projectWithTrack(), selectedTrackId: "t1" });
    holder.value = value;
    render(<TrackInspector />);
    fireEvent.change(screen.getByLabelText("リバーブ送り"), { target: { value: "0.5" } });
    expect(value.updateTrack).toHaveBeenCalledWith("t1", { reverbSend: 0.5 });
  });

  it("edits the maximum simultaneous voice count", () => {
    const value = makeStudioValue({ project: projectWithTrack(), selectedTrackId: "t1" });
    holder.value = value;
    render(<TrackInspector />);
    fireEvent.change(screen.getByRole("spinbutton", { name: /最大同時発音数/ }), { target: { value: "3" } });
    expect(value.updateTrack).toHaveBeenCalledWith("t1", {
      polyphony: { maxVoices: 3, priority: "newest", stopMode: "none" },
    });
  });

  it("clamps the voice count to the supported range", () => {
    const value = makeStudioValue({ project: projectWithTrack(), selectedTrackId: "t1" });
    holder.value = value;
    render(<TrackInspector />);
    fireEvent.change(screen.getByRole("spinbutton", { name: /最大同時発音数/ }), { target: { value: "200" } });
    expect(value.updateTrack).toHaveBeenCalledWith("t1", {
      polyphony: { maxVoices: 64, priority: "newest", stopMode: "none" },
    });
  });

  it("edits the playback priority", () => {
    const value = makeStudioValue({ project: projectWithTrack(), selectedTrackId: "t1" });
    holder.value = value;
    render(<TrackInspector />);
    fireEvent.change(screen.getByRole("combobox", { name: /優先再生/ }), { target: { value: "oldest" } });
    expect(value.updateTrack).toHaveBeenCalledWith("t1", {
      polyphony: { maxVoices: 0, priority: "oldest", stopMode: "none" },
    });
  });

  it("edits the stop method", () => {
    const value = makeStudioValue({ project: projectWithTrack(), selectedTrackId: "t1" });
    holder.value = value;
    render(<TrackInspector />);
    fireEvent.change(screen.getByRole("combobox", { name: /停止方法/ }), { target: { value: "pitch" } });
    expect(value.updateTrack).toHaveBeenCalledWith("t1", {
      polyphony: { maxVoices: 0, priority: "newest", stopMode: "pitch" },
    });
  });

  it("shows when the voice count is unlimited", () => {
    holder.value = makeStudioValue({ project: projectWithTrack(), selectedTrackId: "t1" });
    render(<TrackInspector />);
    expect(screen.getByText("無制限")).toBeInTheDocument();
  });

  it("shows the configured voice count", () => {
    holder.value = makeStudioValue({
      project: projectWithTrack({ polyphony: { maxVoices: 4, priority: "newest", stopMode: "none" } }),
      selectedTrackId: "t1",
    });
    render(<TrackInspector />);
    expect(screen.getByText("4 音")).toBeInTheDocument();
  });

  it("renders pan labels for center, left and right", () => {
    holder.value = makeStudioValue({ project: projectWithTrack({ pan: 0 }), selectedTrackId: "t1" });
    const { rerender } = render(<TrackInspector />);
    expect(screen.getByText("C")).toBeInTheDocument();

    holder.value = makeStudioValue({ project: projectWithTrack({ pan: -0.5 }), selectedTrackId: "t1" });
    rerender(<TrackInspector />);
    expect(screen.getByText("L50")).toBeInTheDocument();

    holder.value = makeStudioValue({ project: projectWithTrack({ pan: 0.25 }), selectedTrackId: "t1" });
    rerender(<TrackInspector />);
    expect(screen.getByText("R25")).toBeInTheDocument();
  });

  it("reflects expression automation in the hint", () => {
    holder.value = makeStudioValue({
      project: projectWithTrack({ dynamics: { volume: [], expression: [{ t: 0, v: 1 }] } }),
      selectedTrackId: "t1",
    });
    render(<TrackInspector />);
    expect(screen.getByText(/エクスプレッション\(CC11\)/)).toBeInTheDocument();
  });

  it("assigns and clears per-note sample overrides", () => {
    const value = makeStudioValue({ project: projectWithTrack(), selectedTrackId: "t1" });
    holder.value = value;
    render(<TrackInspector />);

    const row60 = screen.getByText("C4").closest(".notemap__row")!;
    fireEvent.change(within(row60 as HTMLElement).getByRole("combobox"), { target: { value: "s2" } });
    expect(value.setNoteSample).toHaveBeenCalledWith("t1", 60, "s2");
    expect(value.selectSample).toHaveBeenCalledWith("s2");

    const row67 = screen.getByText("G4").closest(".notemap__row")!;
    fireEvent.change(within(row67 as HTMLElement).getByRole("combobox"), { target: { value: "" } });
    expect(value.setNoteSample).toHaveBeenCalledWith("t1", 67, null);
  });

  it("shows the empty default-sample option when none is assigned", () => {
    holder.value = makeStudioValue({ project: projectWithTrack({ defaultSampleId: null }), selectedTrackId: "t1" });
    render(<TrackInspector />);
    const select = screen.getByRole("combobox", { name: /既定の音声素材/ }) as HTMLSelectElement;
    expect(select.value).toBe("");
  });

  it("notes when the track has no notes to map", () => {
    holder.value = makeStudioValue({ project: projectWithTrack({ notes: [] }), selectedTrackId: "t1" });
    render(<TrackInspector />);
    expect(screen.getByText("ノートがありません。")).toBeInTheDocument();
  });
});
