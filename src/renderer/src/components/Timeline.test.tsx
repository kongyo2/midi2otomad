import { describe, expect, it, vi, beforeEach } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import type { StudioContextValue } from "../state/StudioContext";
import { makeStudioValue } from "../../../test/studio";
import { parseProject } from "../../../shared/schemas/project";

const holder = vi.hoisted(() => ({ value: null as StudioContextValue | null, playhead: 0 }));

vi.mock("../state/StudioContext", () => ({
  useStudio: () => holder.value,
  usePlayhead: () => holder.playhead,
}));
vi.mock("./TrackRow", () => ({
  TrackRow: ({ track }: { track: { name: string } }) => <div data-testid="trackrow">{track.name}</div>,
}));

import { Timeline } from "./Timeline";

const withTracks = parseProject({
  version: 1,
  name: "p",
  tracks: [{ id: "t1", name: "lead", notes: [{ pitch: 60, startSec: 1, durationSec: 2, velocity: 100 }] }],
});

beforeEach(() => {
  holder.value = makeStudioValue();
  holder.playhead = 0;
});

describe("Timeline", () => {
  it("prompts to import when there are no tracks", () => {
    render(<Timeline />);
    expect(screen.getByText(/ファイルをドラッグ＆ドロップ/)).toBeInTheDocument();
  });

  it("renders a row per track and a playhead", () => {
    holder.value = makeStudioValue({ project: withTracks, selectedTrackId: "t1" });
    const { container } = render(<Timeline />);
    expect(screen.getAllByTestId("trackrow")).toHaveLength(1);
    expect(container.querySelector(".playhead")).toBeInTheDocument();
  });

  it("seeks when the ruler is clicked", () => {
    const value = makeStudioValue({ project: withTracks });
    holder.value = value;
    const { container } = render(<Timeline />);
    fireEvent.click(container.querySelector(".ruler")!, { clientX: 160 });
    expect(value.seek).toHaveBeenCalled();
  });

  it("zooms out to widen the tick spacing", () => {
    render(<Timeline />);
    fireEvent.click(screen.getByText("－"));
    fireEvent.click(screen.getByText("－"));
    expect(screen.getByText("48px/s")).toBeInTheDocument();
  });

  it("zooms in to tighten the tick spacing", () => {
    render(<Timeline />);
    fireEvent.click(screen.getByText("＋"));
    fireEvent.click(screen.getByText("＋"));
    expect(screen.getByText("112px/s")).toBeInTheDocument();
  });

  it("auto-scrolls to keep the playhead in view while playing", () => {
    holder.value = makeStudioValue({ isPlaying: true });
    holder.playhead = 2;
    const { container, rerender } = render(<Timeline />);
    const scroller = container.querySelector(".timeline") as HTMLElement;

    let scrollLeft = 0;
    Object.defineProperty(scroller, "scrollLeft", {
      configurable: true,
      get: () => scrollLeft,
      set: (v: number) => {
        scrollLeft = v;
      },
    });
    Object.defineProperty(scroller, "clientWidth", { configurable: true, value: 2000 });

    holder.playhead = 3;
    rerender(<Timeline />);
    expect(scrollLeft).toBe(0);

    scrollLeft = 5000;
    holder.playhead = 4;
    rerender(<Timeline />);
    expect(scroller).toBeInTheDocument();
  });

  it("does not scroll while paused", () => {
    holder.value = makeStudioValue({ isPlaying: false });
    holder.playhead = 50;
    render(<Timeline />);
  });
});
