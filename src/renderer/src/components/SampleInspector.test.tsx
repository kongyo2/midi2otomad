import { describe, expect, it, vi, beforeEach } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import type { StudioContextValue } from "../state/StudioContext";
import { makeStudioValue } from "../../../test/studio";
import { parseProject } from "../../../shared/schemas/project";
import type { PcmAudio } from "../../../shared/audio/mixer";

const holder = vi.hoisted(() => ({ value: null as StudioContextValue | null }));
const previewSample = vi.hoisted(() => vi.fn());

vi.mock("../state/StudioContext", () => ({
  useStudio: () => holder.value,
}));
vi.mock("../audio/preview", () => ({ previewSample }));
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
});

describe("SampleInspector", () => {
  it("shows a placeholder when no sample is selected", () => {
    render(<SampleInspector />);
    expect(screen.getByText(/ライブラリから音声素材を選択/)).toBeInTheDocument();
  });

  it("edits the name", () => {
    const value = withSample();
    holder.value = value;
    render(<SampleInspector />);
    fireEvent.change(screen.getByDisplayValue("Kick"), { target: { value: "Boom" } });
    expect(value.updateSample).toHaveBeenCalledWith("s1", { name: "Boom" });
  });

  it("edits base pitch, tune and gain", () => {
    const value = withSample();
    holder.value = value;
    render(<SampleInspector />);
    fireEvent.change(screen.getByLabelText("基準ピッチ"), { target: { value: "48" } });
    expect(value.updateSample).toHaveBeenCalledWith("s1", { basePitch: 48 });
    fireEvent.change(screen.getByLabelText("微調整"), { target: { value: "20" } });
    expect(value.updateSample).toHaveBeenCalledWith("s1", { tuneCents: 20 });
    fireEvent.change(screen.getByLabelText("ゲイン"), { target: { value: "2" } });
    expect(value.updateSample).toHaveBeenCalledWith("s1", { gain: 2 });
  });

  it("switches the interpolation mode", () => {
    const value = withSample();
    holder.value = value;
    render(<SampleInspector />);
    fireEvent.change(screen.getByLabelText("補間方式"), { target: { value: "linear" } });
    expect(value.updateSample).toHaveBeenCalledWith("s1", { interpolation: "linear" });
  });

  it("edits the envelope stages while preserving the others", () => {
    const value = withSample();
    holder.value = value;
    render(<SampleInspector />);
    fireEvent.change(screen.getByLabelText("アタック"), { target: { value: "10" } });
    expect(value.updateSample).toHaveBeenCalledWith("s1", {
      envelope: expect.objectContaining({ attackMs: 10, releaseMs: 90, sustain: 1 }),
    });
    fireEvent.change(screen.getByLabelText("サステイン"), { target: { value: "0.4" } });
    expect(value.updateSample).toHaveBeenCalledWith("s1", {
      envelope: expect.objectContaining({ sustain: 0.4, attackMs: 4 }),
    });
    fireEvent.change(screen.getByLabelText("ディケイ"), { target: { value: "120" } });
    expect(value.updateSample).toHaveBeenCalledWith("s1", {
      envelope: expect.objectContaining({ decayMs: 120 }),
    });
  });

  it("edits an envelope curve", () => {
    const value = withSample();
    holder.value = value;
    render(<SampleInspector />);
    fireEvent.change(screen.getByLabelText("アタックカーブ"), { target: { value: "3" } });
    expect(value.updateSample).toHaveBeenCalledWith("s1", {
      envelope: expect.objectContaining({ attackCurve: 3 }),
    });
  });

  it("toggles and configures the timbre filter", () => {
    const value = withSample();
    holder.value = value;
    render(<SampleInspector />);
    fireEvent.click(screen.getByLabelText("フィルター"));
    expect(value.updateSample).toHaveBeenCalledWith("s1", {
      filter: expect.objectContaining({ enabled: true }),
    });
    fireEvent.change(screen.getByLabelText("フィルタータイプ"), { target: { value: "highpass" } });
    expect(value.updateSample).toHaveBeenCalledWith("s1", {
      filter: expect.objectContaining({ type: "highpass" }),
    });
    fireEvent.change(screen.getByLabelText("カットオフ"), { target: { value: "1200" } });
    expect(value.updateSample).toHaveBeenCalledWith("s1", {
      filter: expect.objectContaining({ cutoffHz: 1200 }),
    });
    fireEvent.change(screen.getByLabelText("レゾナンス"), { target: { value: "4" } });
    expect(value.updateSample).toHaveBeenCalledWith("s1", {
      filter: expect.objectContaining({ q: 4 }),
    });
  });

  it("modulates the filter cutoff with an envelope and an LFO", () => {
    const value = withSample();
    holder.value = value;
    render(<SampleInspector />);
    fireEvent.change(screen.getByLabelText("フィルターEG"), { target: { value: "3" } });
    expect(value.updateSample).toHaveBeenCalledWith("s1", {
      filter: expect.objectContaining({ envAmount: 3 }),
    });
    fireEvent.change(screen.getByLabelText("フィルターLFO深さ"), { target: { value: "2" } });
    expect(value.updateSample).toHaveBeenCalledWith("s1", {
      filter: expect.objectContaining({ lfoDepth: 2 }),
    });
    fireEvent.change(screen.getByLabelText("フィルターLFO波形"), { target: { value: "square" } });
    expect(value.updateSample).toHaveBeenCalledWith("s1", {
      filter: expect.objectContaining({ lfoShape: "square" }),
    });
  });

  it("configures pitch glide and vibrato", () => {
    const value = withSample();
    holder.value = value;
    render(<SampleInspector />);
    fireEvent.change(screen.getByLabelText("グライド量"), { target: { value: "-12" } });
    expect(value.updateSample).toHaveBeenCalledWith("s1", {
      pitchMod: expect.objectContaining({ glideSemitones: -12 }),
    });
    fireEvent.change(screen.getByLabelText("ビブラート深さ"), { target: { value: "40" } });
    expect(value.updateSample).toHaveBeenCalledWith("s1", {
      pitchMod: expect.objectContaining({ vibratoCents: 40 }),
    });
    fireEvent.change(screen.getByLabelText("ビブラート波形"), { target: { value: "triangle" } });
    expect(value.updateSample).toHaveBeenCalledWith("s1", {
      pitchMod: expect.objectContaining({ vibratoShape: "triangle" }),
    });
  });

  it("exposes the glide curve and vibrato fade controls", () => {
    const value = withSample();
    holder.value = value;
    render(<SampleInspector />);
    fireEvent.change(screen.getByLabelText("グライドカーブ"), { target: { value: "2" } });
    expect(value.updateSample).toHaveBeenCalledWith("s1", {
      pitchMod: expect.objectContaining({ glideCurve: 2 }),
    });
    fireEvent.change(screen.getByLabelText("ビブラートフェード"), { target: { value: "300" } });
    expect(value.updateSample).toHaveBeenCalledWith("s1", {
      pitchMod: expect.objectContaining({ vibratoFadeMs: 300 }),
    });
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

  it("toggles the loop and resets it to the full sample", () => {
    const value = withSample();
    holder.value = value;
    render(<SampleInspector />);

    fireEvent.click(screen.getByRole("checkbox", { name: /ループ/ }));
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
