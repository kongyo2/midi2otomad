import { afterEach, describe, expect, it, vi, beforeEach } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import type { StudioContextValue } from "../state/StudioContext";
import { makeStudioValue } from "../../../test/studio";
import { parseProject } from "../../../shared/schemas/project";

const holder = vi.hoisted(() => ({ value: null as StudioContextValue | null, playhead: 0 }));

vi.mock("../state/StudioContext", () => ({
  useStudio: () => holder.value,
  usePlayhead: () => holder.playhead,
}));

import { TopBar } from "./TopBar";

const projectWithNotes = parseProject({
  version: 1,
  name: "song",
  bpm: 128.4,
  masterGain: 1,
  tracks: [
    {
      id: "t1",
      name: "lead",
      notes: [{ pitch: 60, startSec: 1, durationSec: 2, velocity: 100 }],
    },
  ],
});

beforeEach(() => {
  holder.value = makeStudioValue({ project: projectWithNotes });
  holder.playhead = 0;
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("TopBar", () => {
  it("drives the transport controls", () => {
    const value = holder.value!;
    render(<TopBar />);
    fireEvent.click(screen.getByTitle("先頭へ"));
    expect(value.seek).toHaveBeenCalledWith(0);
    fireEvent.click(screen.getByText("▶"));
    expect(value.togglePlay).toHaveBeenCalled();
    fireEvent.click(screen.getByTitle("停止"));
    expect(value.stop).toHaveBeenCalled();
  });

  it("shows a pause glyph while playing and the rounded bpm", () => {
    holder.value = makeStudioValue({ project: projectWithNotes, isPlaying: true });
    render(<TopBar />);
    expect(screen.getByText("⏸")).toBeInTheDocument();
    expect(screen.getByText("128")).toBeInTheDocument();
  });

  it("adjusts the master gain", () => {
    const value = holder.value!;
    render(<TopBar />);
    fireEvent.change(screen.getByRole("slider"), { target: { value: "1.5" } });
    expect(value.patchProject).toHaveBeenCalledWith({ masterGain: 1.5 });
  });

  it("opens a MIDI file when one is chosen", async () => {
    const openMidi = vi.fn(async () => ({ name: "x.mid", data: new Uint8Array([1]) }));
    vi.stubGlobal("api", { openMidi });
    const value = holder.value!;
    render(<TopBar />);
    fireEvent.click(screen.getByText("MIDI を開く"));
    await vi.waitFor(() => expect(value.importMidiBytes).toHaveBeenCalledWith(new Uint8Array([1]), "x.mid"));
  });

  it("ignores a canceled MIDI dialog", async () => {
    const openMidi = vi.fn(async () => null);
    vi.stubGlobal("api", { openMidi });
    const value = holder.value!;
    render(<TopBar />);
    fireEvent.click(screen.getByText("MIDI を開く"));
    await vi.waitFor(() => expect(openMidi).toHaveBeenCalled());
    expect(value.importMidiBytes).not.toHaveBeenCalled();
  });

  it("exports WAV with the chosen bit depth", () => {
    const value = holder.value!;
    render(<TopBar />);
    fireEvent.change(screen.getByDisplayValue("24-bit"), { target: { value: "16" } });
    fireEvent.click(screen.getByText("⬇ 書き出し"));
    expect(value.exportMix).toHaveBeenCalledWith({ format: "wav", wavBitDepth: 16 });
  });

  it("switches to MP3 and exports with the chosen bitrate", () => {
    const value = holder.value!;
    render(<TopBar />);
    fireEvent.change(screen.getByDisplayValue("WAV"), { target: { value: "mp3" } });
    fireEvent.change(screen.getByDisplayValue("320k"), { target: { value: "256" } });
    fireEvent.click(screen.getByText("⬇ 書き出し"));
    expect(value.exportMix).toHaveBeenCalledWith({ format: "mp3", mp3Bitrate: 256 });
  });

  it("disables export while busy", () => {
    holder.value = makeStudioValue({ project: projectWithNotes, busy: "処理中" });
    render(<TopBar />);
    const button = screen.getByText("処理中…");
    expect(button).toBeDisabled();
  });
});
