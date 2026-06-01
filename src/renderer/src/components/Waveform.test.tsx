import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render } from "@testing-library/react";
import { Waveform } from "./Waveform";

function setDpr(value: number): void {
  Object.defineProperty(window, "devicePixelRatio", { value, configurable: true });
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("Waveform", () => {
  it("draws peaks on a high-DPR canvas", () => {
    setDpr(2);
    const { container } = render(<Waveform peaks={new Float32Array([0.2, 0.8, 0.5])} />);
    expect(container.querySelector("canvas")).toBeInTheDocument();
  });

  it("falls back to a device pixel ratio of 1", () => {
    setDpr(0);
    const { container } = render(<Waveform peaks={new Float32Array([0.4, 0.6])} height={120} color="#fff" />);
    expect(container.querySelector("canvas")).toBeInTheDocument();
  });

  it("draws an enabled loop region", () => {
    setDpr(1);
    render(<Waveform peaks={new Float32Array([0.3])} loop={{ startFrac: 0.2, endFrac: 0.7, enabled: true }} />);
  });

  it("skips a disabled loop region", () => {
    setDpr(1);
    render(<Waveform peaks={new Float32Array([0.3])} loop={{ startFrac: 0.2, endFrac: 0.7, enabled: false }} />);
  });

  it("renders without peak data", () => {
    setDpr(1);
    render(<Waveform peaks={undefined} />);
  });

  it("renders with an empty peak array", () => {
    setDpr(1);
    render(<Waveform peaks={new Float32Array(0)} />);
  });

  it("aborts drawing when no 2d context is available", () => {
    setDpr(1);
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
    render(<Waveform peaks={new Float32Array([0.5])} />);
  });

  it("redraws on window resize", () => {
    setDpr(1);
    render(<Waveform peaks={new Float32Array([0.5])} />);
    fireEvent(window, new Event("resize"));
  });
});
