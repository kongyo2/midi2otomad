import { afterEach, describe, expect, it, vi, beforeEach } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import type { StudioContextValue } from "../state/StudioContext";
import { makeStudioValue } from "../../../test/studio";
import { parseProject } from "../../../shared/schemas/project";

const holder = vi.hoisted(() => ({ value: null as StudioContextValue | null }));

vi.mock("../state/StudioContext", () => ({
  useStudio: () => holder.value,
}));

import { SampleLibrary } from "./SampleLibrary";

const projectWithSamples = parseProject({
  version: 1,
  name: "p",
  samples: [
    { id: "s1", name: "Kick", basePitch: 60, durationSec: 1.5, loop: { enabled: true, startSec: 0, endSec: 1 } },
    { id: "s2", name: "Snare", basePitch: 62, durationSec: 0.5 },
  ],
});

beforeEach(() => {
  holder.value = makeStudioValue();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("SampleLibrary", () => {
  it("shows an empty hint when there are no samples", () => {
    render(<SampleLibrary />);
    expect(screen.getByText(/ここにドロップ/)).toBeInTheDocument();
  });

  it("opens the file dialog and ingests the chosen audio", async () => {
    const openAudio = vi.fn(async () => [{ name: "a.wav", data: new Uint8Array() }]);
    vi.stubGlobal("api", { openAudio });
    const value = makeStudioValue();
    holder.value = value;
    render(<SampleLibrary />);

    fireEvent.click(screen.getByRole("button", { name: "+ 追加" }));
    await vi.waitFor(() => expect(value.ingestAudio).toHaveBeenCalledTimes(1));
  });

  it("ignores a canceled file dialog", async () => {
    const openAudio = vi.fn(async () => null);
    vi.stubGlobal("api", { openAudio });
    const value = makeStudioValue();
    holder.value = value;
    render(<SampleLibrary />);

    fireEvent.click(screen.getByRole("button", { name: "+ 追加" }));
    await vi.waitFor(() => expect(openAudio).toHaveBeenCalled());
    expect(value.ingestAudio).not.toHaveBeenCalled();
  });

  it("toggles the drag-over state and ingests dropped audio files", () => {
    const value = makeStudioValue();
    holder.value = value;
    const { container } = render(<SampleLibrary />);
    const droparea = container.querySelector(".droparea")!;

    fireEvent.dragOver(droparea);
    expect(droparea.className).toContain("droparea--over");
    fireEvent.dragLeave(droparea);
    expect(droparea.className).not.toContain("droparea--over");

    fireEvent.drop(droparea, { dataTransfer: { files: [new File(["x"], "loop.wav")] } });
    expect(value.ingestAudio).toHaveBeenCalledTimes(1);
  });

  it("ignores a drop that contains only MIDI files", () => {
    const value = makeStudioValue();
    holder.value = value;
    const { container } = render(<SampleLibrary />);
    const droparea = container.querySelector(".droparea")!;

    fireEvent.drop(droparea, { dataTransfer: { files: [new File(["x"], "song.mid")] } });
    expect(value.ingestAudio).not.toHaveBeenCalled();
  });

  it("lists samples with waveforms, selection and removal", () => {
    const getPeaks = vi.fn((id: string) => (id === "s1" ? new Float32Array([0.1, 0.2, 0.3, 0.4, 0.5]) : undefined));
    const value = makeStudioValue({ project: projectWithSamples, selectedSampleId: "s1", getPeaks });
    holder.value = value;
    render(<SampleLibrary />);

    expect(screen.getByText("Kick")).toBeInTheDocument();
    expect(screen.getByText(/⟳loop/)).toBeInTheDocument();
    const active = document.querySelector(".samplelist__item--active");
    expect(active?.textContent).toContain("Kick");

    fireEvent.click(screen.getByText("Snare"));
    expect(value.selectSample).toHaveBeenCalledWith("s2");

    fireEvent.click(screen.getAllByRole("button", { name: "✕" })[0]!);
    expect(value.removeSample).toHaveBeenCalledWith("s1");
  });
});
