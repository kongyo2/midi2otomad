import { afterEach, describe, expect, it, vi, beforeEach } from "vitest";
import { fireEvent, render, screen, within } from "@testing-library/react";
import type { StudioContextValue } from "../state/StudioContext";
import { makeStudioValue } from "../../../test/studio";
import { parseProject, type Track } from "../../../shared/schemas/project";

const holder = vi.hoisted(() => ({ value: null as StudioContextValue | null }));

vi.mock("../state/StudioContext", () => ({
  useStudio: () => holder.value,
}));

import { TrackRow } from "./TrackRow";

function track(over: Record<string, unknown> = {}): Track {
  return parseProject({
    version: 1,
    name: "p",
    samples: [{ id: "s1", name: "Kick" }],
    tracks: [
      {
        id: "t1",
        name: "Lead",
        color: "#36d399",
        defaultSampleId: "s1",
        noteSampleMap: { "60": "s1" },
        notes: [
          { pitch: 60, startSec: 0, durationSec: 1, velocity: 100 },
          { pitch: 64, startSec: 0.5, durationSec: 0.5, velocity: 60 },
          { pitch: 62, startSec: 100, durationSec: 1, velocity: 80 },
        ],
        ...over,
      },
    ],
  }).tracks[0]!;
}

function renderRow(t: Track, over: Partial<React.ComponentProps<typeof TrackRow>> = {}) {
  return render(<TrackRow track={t} pxPerSec={80} rowHeight={96} canvasWidth={400} selected={false} {...over} />);
}

beforeEach(() => {
  holder.value = makeStudioValue({
    project: parseProject({ version: 1, name: "p", samples: [{ id: "s1", name: "Kick" }] }),
  });
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("TrackRow", () => {
  it("paints the piano-roll notes onto the canvas", () => {
    const { container } = renderRow(track(), { selected: true });
    expect(container.querySelector(".trackrow--selected")).toBeInTheDocument();
    expect(container.querySelector("canvas")).toBeInTheDocument();
  });

  it("clears the canvas for a track with no notes", () => {
    renderRow(track({ notes: [] }));
  });

  it("aborts when no 2d context is available", () => {
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
    renderRow(track());
  });

  it("selects the track and toggles mute / solo", () => {
    const value = holder.value!;
    renderRow(track());
    fireEvent.click(screen.getByTitle("トラックを選択"));
    expect(value.selectTrack).toHaveBeenCalledWith("t1");
    fireEvent.click(screen.getByText("M"));
    expect(value.updateTrack).toHaveBeenCalledWith("t1", { muted: true });
    fireEvent.click(screen.getByText("S"));
    expect(value.updateTrack).toHaveBeenCalledWith("t1", { solo: true });
  });

  it("changes the default sample, including clearing it", () => {
    const value = holder.value!;
    renderRow(track());
    const select = screen.getByRole("combobox");
    fireEvent.change(select, { target: { value: "" } });
    expect(value.updateTrack).toHaveBeenCalledWith("t1", { defaultSampleId: null });
    fireEvent.change(select, { target: { value: "s1" } });
    expect(value.updateTrack).toHaveBeenCalledWith("t1", { defaultSampleId: "s1" });
  });

  it("seeks when the lane is clicked", () => {
    const value = holder.value!;
    const { container } = renderRow(track());
    fireEvent.click(container.querySelector(".trackrow__lane")!);
    expect(value.seek).toHaveBeenCalled();
  });

  it("highlights a soloed track's button", () => {
    const { container } = renderRow(track({ solo: true }));
    expect(container.querySelector(".tag--solo")).toBeInTheDocument();
  });

  it("dims a muted track", () => {
    const { container } = renderRow(track({ muted: true }));
    expect(container.querySelector(".trackrow__lane--dim")).toBeInTheDocument();
  });

  it("dims non-soloed tracks when another track is soloed", () => {
    const project = parseProject({
      version: 1,
      name: "p",
      samples: [{ id: "s1", name: "Kick" }],
      tracks: [
        { id: "t1", name: "A", notes: [], solo: false },
        { id: "t2", name: "B", notes: [], solo: true },
      ],
    });
    holder.value = makeStudioValue({ project });
    const { container } = renderRow(project.tracks[0]!);
    expect(container.querySelector(".trackrow__lane--dim")).toBeInTheDocument();
    expect(within(container).getByText("A")).toBeInTheDocument();
  });
});
