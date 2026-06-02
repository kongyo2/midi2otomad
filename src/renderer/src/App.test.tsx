import { describe, expect, it, vi, beforeEach } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { StudioContextValue } from "./state/StudioContext";
import { makeStudioValue } from "../../test/studio";

const holder = vi.hoisted(() => ({ value: null as StudioContextValue | null }));

vi.mock("./state/StudioContext", () => ({
  useStudio: () => holder.value,
}));
vi.mock("./components/TopBar", () => ({ TopBar: () => <div data-testid="topbar" /> }));
vi.mock("./components/SampleLibrary", () => ({ SampleLibrary: () => <div data-testid="lib" /> }));
vi.mock("./components/SampleInspector", () => ({ SampleInspector: () => <div data-testid="inspector" /> }));
vi.mock("./components/Timeline", () => ({ Timeline: () => <div data-testid="timeline" /> }));
vi.mock("./components/TrackInspector", () => ({ TrackInspector: () => <div data-testid="trackinspector" /> }));
vi.mock("./components/ReverbPanel", () => ({ ReverbPanel: () => <div data-testid="reverb" /> }));
vi.mock("./components/OutputPanel", () => ({ OutputPanel: () => <div data-testid="output" /> }));
vi.mock("./components/HelpPanel", () => ({ HelpPanel: () => <div data-testid="help" /> }));

import { App } from "./App";

function midiFile(name = "song.mid"): File {
  return new File([new Uint8Array([1, 2, 3])], name);
}
function audioFile(name = "kick.wav"): File {
  return new File([new Uint8Array([4, 5])], name);
}

beforeEach(() => {
  holder.value = makeStudioValue();
});

describe("App", () => {
  it("renders the studio layout", () => {
    render(<App />);
    expect(screen.getByTestId("topbar")).toBeInTheDocument();
    expect(screen.getByTestId("timeline")).toBeInTheDocument();
    expect(screen.getByTestId("output")).toBeInTheDocument();
  });

  it("toggles playback on Space but ignores form fields and other keys", () => {
    const value = makeStudioValue();
    holder.value = value;
    render(<App />);

    fireEvent.keyDown(document.body, { code: "Space" });
    expect(value.togglePlay).toHaveBeenCalledTimes(1);

    fireEvent.keyDown(window, { code: "Space" });
    expect(value.togglePlay).toHaveBeenCalledTimes(2);

    fireEvent.keyDown(document.body, { code: "KeyA" });
    expect(value.togglePlay).toHaveBeenCalledTimes(2);

    const input = document.createElement("input");
    document.body.appendChild(input);
    fireEvent.keyDown(input, { code: "Space" });
    expect(value.togglePlay).toHaveBeenCalledTimes(2);
    input.remove();
  });

  it("keeps the overlay while nested drag-enters outnumber leaves", () => {
    render(<App />);
    const studio = document.querySelector(".studio")!;
    fireEvent.dragEnter(studio);
    fireEvent.dragEnter(studio);
    fireEvent.dragLeave(studio);
    expect(screen.getByText("ここにドロップ")).toBeInTheDocument();
  });

  it("shows the drop overlay on drag enter and hides it on leave", () => {
    render(<App />);
    const studio = document.querySelector(".studio")!;
    fireEvent.dragEnter(studio);
    expect(screen.getByText("ここにドロップ")).toBeInTheDocument();
    fireEvent.dragOver(studio);
    fireEvent.dragLeave(studio);
    expect(screen.queryByText("ここにドロップ")).not.toBeInTheDocument();
  });

  it("imports MIDI and ingests audio from a mixed drop", async () => {
    const value = makeStudioValue();
    holder.value = value;
    render(<App />);
    const studio = document.querySelector(".studio")!;
    fireEvent.drop(studio, { dataTransfer: { files: [midiFile(), audioFile()] } });
    await waitFor(() => expect(value.importMidiBytes).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(value.ingestAudio).toHaveBeenCalledTimes(1));
  });

  it("imports MIDI without ingesting when no audio is dropped", async () => {
    const value = makeStudioValue();
    holder.value = value;
    render(<App />);
    const studio = document.querySelector(".studio")!;
    fireEvent.drop(studio, { dataTransfer: { files: [midiFile()] } });
    await waitFor(() => expect(value.importMidiBytes).toHaveBeenCalledTimes(1));
    expect(value.ingestAudio).not.toHaveBeenCalled();
  });

  it("ingests audio without importing when no MIDI is dropped", async () => {
    const value = makeStudioValue();
    holder.value = value;
    render(<App />);
    const studio = document.querySelector(".studio")!;
    fireEvent.drop(studio, { dataTransfer: { files: [audioFile()] } });
    await waitFor(() => expect(value.ingestAudio).toHaveBeenCalledTimes(1));
    expect(value.importMidiBytes).not.toHaveBeenCalled();
  });

  it("renders the busy bar and toast", () => {
    holder.value = makeStudioValue({ busy: "処理中…", toast: "完了" });
    render(<App />);
    expect(screen.getByText("処理中…")).toBeInTheDocument();
    expect(screen.getByText("完了")).toBeInTheDocument();
  });
});
