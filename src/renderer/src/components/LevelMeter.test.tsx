import { afterEach, describe, expect, it, vi, beforeEach } from "vitest";
import { act, render } from "@testing-library/react";
import type { StudioContextValue } from "../state/StudioContext";
import { makeStudioValue } from "../../../test/studio";
import type { PreviewEngine } from "../audio/engine";

const holder = vi.hoisted(() => ({ value: null as StudioContextValue | null }));

vi.mock("../state/StudioContext", () => ({
  useStudio: () => holder.value,
}));

import { LevelMeter } from "./LevelMeter";

function fakeEngine(fftSize: number): { engine: PreviewEngine; analyser: { fftSize: number } } {
  const analyser = {
    fftSize,
    getFloatTimeDomainData: (buffer: Float32Array) => {
      buffer.fill(0.5);
    },
  };
  const engine = { getMasterAnalyser: () => analyser } as unknown as PreviewEngine;
  return { engine, analyser };
}

beforeEach(() => {
  holder.value = makeStudioValue();
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("LevelMeter", () => {
  it("reads the analyser across frames and reuses or reallocates the buffer", () => {
    const { engine, analyser } = fakeEngine(4);
    holder.value = makeStudioValue({ engineRef: { current: engine } });
    const rafSpy = vi.spyOn(window, "requestAnimationFrame");
    const { container } = render(<LevelMeter />);

    const tick = rafSpy.mock.calls[0]![0];
    act(() => {
      tick(0);
    });
    const fill = container.querySelector(".meter__fill") as HTMLElement;
    expect(fill.style.width).toBe("80%");

    act(() => {
      tick(0);
    });
    analyser.fftSize = 8;
    act(() => {
      tick(0);
    });
    expect(fill.style.width).toBe("80%");
  });

  it("stays idle when no engine is present", () => {
    holder.value = makeStudioValue({ engineRef: { current: null } });
    const rafSpy = vi.spyOn(window, "requestAnimationFrame");
    const { container } = render(<LevelMeter />);

    const tick = rafSpy.mock.calls[0]![0];
    act(() => {
      tick(0);
    });
    const fill = container.querySelector(".meter__fill") as HTMLElement;
    expect(fill.style.width).toBe("0%");
  });
});
