import { describe, expect, it, vi, beforeEach } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import type { StudioContextValue } from "../state/StudioContext";
import { makeStudioValue } from "../../../test/studio";
import { parseProject } from "../../../shared/schemas/project";
import type { PcmAudio } from "../../../shared/audio/mixer";

const holder = vi.hoisted(() => ({ value: null as StudioContextValue | null }));
const previewSample = vi.hoisted(() => vi.fn());
const detectSamplePitch = vi.hoisted(() => vi.fn());

vi.mock("../state/StudioContext", () => ({
  useStudio: () => holder.value,
}));
vi.mock("../audio/preview", () => ({ previewSample }));
vi.mock("../../../shared/music/detect", () => ({ detectSamplePitch }));
vi.mock("./Waveform", () => ({ Waveform: () => <div data-testid="waveform" /> }));

import { SampleInspector } from "./SampleInspector";

function withSample(over: Record<string, unknown> = {}): StudioContextValue {
  const project = parseProject({
    version: 1,
    name: "p",
    samples: [
      {
        id: "s1",
        name: "Kick",
        basePitch: 60,
        tuneCents: 0,
        gain: 1,
        durationSec: 2,
        loop: { enabled: true, startSec: 0.1, endSec: 0.5 },
        envelope: { attackMs: 4, releaseMs: 90 },
        ...over,
      },
    ],
  });
  return makeStudioValue({ project, selectedSampleId: "s1" });
}

beforeEach(() => {
  holder.value = makeStudioValue();
  previewSample.mockClear();
  detectSamplePitch.mockReset();
});

describe("SampleInspector", () => {
  it("shows a placeholder when no sample is selected", () => {
    render(<SampleInspector />);
    expect(screen.getByText(/ライブラリから音声素材を選択/)).toBeInTheDocument();
  });

  it("edits the sample fields", () => {
    const value = withSample();
    holder.value = value;
    render(<SampleInspector />);

    fireEvent.change(screen.getByDisplayValue("Kick"), { target: { value: "Boom" } });
    expect(value.updateSample).toHaveBeenCalledWith("s1", { name: "Boom" });

    const sliders = screen.getAllByRole("slider");
    fireEvent.change(sliders[0]!, { target: { value: "48" } });
    expect(value.updateSample).toHaveBeenCalledWith("s1", { basePitch: 48 });
    fireEvent.change(sliders[1]!, { target: { value: "20" } });
    expect(value.updateSample).toHaveBeenCalledWith("s1", { tuneCents: 20 });
    fireEvent.change(sliders[2]!, { target: { value: "2" } });
    expect(value.updateSample).toHaveBeenCalledWith("s1", { gain: 2 });
    fireEvent.change(sliders[3]!, { target: { value: "10" } });
    expect(value.updateSample).toHaveBeenCalledWith("s1", { envelope: { attackMs: 10, releaseMs: 90 } });
    fireEvent.change(sliders[4]!, { target: { value: "200" } });
    expect(value.updateSample).toHaveBeenCalledWith("s1", { envelope: { attackMs: 4, releaseMs: 200 } });
  });

  it("previews when decoded audio is available", () => {
    const pcm: PcmAudio = { sampleRate: 48000, channels: [new Float32Array(4)], frames: 4 };
    const value = withSample();
    value.getAudio = vi.fn(() => pcm);
    holder.value = value;
    render(<SampleInspector />);
    fireEvent.click(screen.getByText("▶ 試聴"));
    expect(previewSample).toHaveBeenCalledTimes(1);
  });

  it("does not preview when audio is missing", () => {
    const value = withSample();
    value.getAudio = vi.fn(() => undefined);
    holder.value = value;
    render(<SampleInspector />);
    fireEvent.click(screen.getByText("▶ 試聴"));
    expect(previewSample).not.toHaveBeenCalled();
  });

  it("auto-detects the base pitch and tuning from decoded audio", () => {
    const pcm: PcmAudio = { sampleRate: 48000, channels: [new Float32Array(4)], frames: 4 };
    const value = withSample();
    value.getAudio = vi.fn(() => pcm);
    holder.value = value;
    detectSamplePitch.mockReturnValue({
      frequencyHz: 392,
      midi: 67.12,
      basePitch: 67,
      tuneCents: 12,
      probability: 0.95,
      voicedFrames: 40,
    });
    render(<SampleInspector />);

    fireEvent.click(screen.getByText(/自動検出/));

    expect(detectSamplePitch).toHaveBeenCalledWith(pcm);
    expect(value.updateSample).toHaveBeenCalledWith("s1", { basePitch: 67, tuneCents: 12 });
    expect(value.showToast).toHaveBeenCalledTimes(1);
  });

  it("labels a flat (negative cents) detection without a plus sign", () => {
    const pcm: PcmAudio = { sampleRate: 48000, channels: [new Float32Array(4)], frames: 4 };
    const value = withSample();
    value.getAudio = vi.fn(() => pcm);
    holder.value = value;
    detectSamplePitch.mockReturnValue({
      frequencyHz: 256,
      midi: 59.62,
      basePitch: 60,
      tuneCents: -38,
      probability: 0.9,
      voicedFrames: 30,
    });
    render(<SampleInspector />);

    fireEvent.click(screen.getByText(/自動検出/));

    expect(value.updateSample).toHaveBeenCalledWith("s1", { basePitch: 60, tuneCents: -38 });
    expect(value.showToast).toHaveBeenCalledWith(expect.stringContaining("-38 cent"));
    expect(value.showToast).not.toHaveBeenCalledWith(expect.stringContaining("+-"));
  });

  it("warns and does not change the sample when no audio is decoded", () => {
    const value = withSample();
    value.getAudio = vi.fn(() => undefined);
    holder.value = value;
    render(<SampleInspector />);

    fireEvent.click(screen.getByText(/自動検出/));

    expect(detectSamplePitch).not.toHaveBeenCalled();
    expect(value.updateSample).not.toHaveBeenCalled();
    expect(value.showToast).toHaveBeenCalledTimes(1);
  });

  it("warns and does not change the sample when no pitch is found", () => {
    const pcm: PcmAudio = { sampleRate: 48000, channels: [new Float32Array(4)], frames: 4 };
    const value = withSample();
    value.getAudio = vi.fn(() => pcm);
    holder.value = value;
    detectSamplePitch.mockReturnValue(null);
    render(<SampleInspector />);

    fireEvent.click(screen.getByText(/自動検出/));

    expect(value.updateSample).not.toHaveBeenCalled();
    expect(value.showToast).toHaveBeenCalledTimes(1);
  });

  it("toggles the loop and resets it to the full sample", () => {
    const value = withSample();
    holder.value = value;
    render(<SampleInspector />);

    fireEvent.click(screen.getByRole("checkbox"));
    expect(value.updateSample).toHaveBeenCalledWith("s1", {
      loop: { enabled: false, startSec: 0.1, endSec: 0.5 },
    });

    fireEvent.click(screen.getByText("全体"));
    expect(value.updateSample).toHaveBeenCalledWith("s1", {
      loop: { enabled: true, startSec: 0, endSec: 2 },
    });
  });

  it("drags the loop handles", () => {
    const value = withSample();
    holder.value = value;
    const { container } = render(<SampleInspector />);
    const handles = container.querySelectorAll(".loophandle");
    expect(handles).toHaveLength(2);

    fireEvent.pointerDown(handles[0]!, { clientX: 5 });
    fireEvent.pointerMove(window, { clientX: 25 });
    fireEvent.pointerUp(window);

    fireEvent.pointerDown(handles[1]!, { clientX: 5 });
    fireEvent.pointerMove(window, { clientX: 25 });
    fireEvent.pointerUp(window);

    expect(value.updateSample).toHaveBeenCalled();
    const calls = (value.updateSample as ReturnType<typeof vi.fn>).mock.calls;
    expect(calls.some(([, patch]) => "loop" in patch && "startSec" in patch.loop)).toBe(true);
    expect(calls.some(([, patch]) => "loop" in patch && "endSec" in patch.loop)).toBe(true);
  });

  it("hides the loop handles when looping is disabled", () => {
    const value = withSample({ loop: { enabled: false, startSec: 0.1, endSec: 0.5 } });
    holder.value = value;
    const { container } = render(<SampleInspector />);
    expect(container.querySelectorAll(".loophandle")).toHaveLength(0);
  });

  it("falls back to a unit duration and full loop end for a zero-length sample", () => {
    const value = withSample({ durationSec: 0, loop: { enabled: true, startSec: 0, endSec: 0 } });
    holder.value = value;
    render(<SampleInspector />);
    expect(screen.getByText(/end 1\.000s/)).toBeInTheDocument();
  });
});
