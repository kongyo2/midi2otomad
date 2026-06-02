import { describe, expect, it, vi, beforeEach } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import type { StudioContextValue } from "../state/StudioContext";
import { makeStudioValue } from "../../../test/studio";

const holder = vi.hoisted(() => ({ value: null as StudioContextValue | null }));
vi.mock("../state/StudioContext", () => ({ useStudio: () => holder.value }));

import { OutputPanel } from "./OutputPanel";

beforeEach(() => {
  holder.value = makeStudioValue();
});

describe("OutputPanel", () => {
  it("selects the render sample rate", () => {
    const value = makeStudioValue();
    holder.value = value;
    render(<OutputPanel />);
    fireEvent.change(screen.getByLabelText("サンプルレート"), { target: { value: "44100" } });
    expect(value.patchProject).toHaveBeenCalledWith({ sampleRate: 44100 });
  });

  it("adjusts the trailing tail length", () => {
    const value = makeStudioValue();
    holder.value = value;
    render(<OutputPanel />);
    fireEvent.change(screen.getByLabelText("テール"), { target: { value: "1.5" } });
    expect(value.patchProject).toHaveBeenCalledWith({
      output: expect.objectContaining({ tailSec: 1.5 }),
    });
  });

  it("toggles the master limiter", () => {
    const value = makeStudioValue();
    holder.value = value;
    render(<OutputPanel />);
    fireEvent.click(screen.getByLabelText("リミッター"));
    expect(value.patchProject).toHaveBeenCalledWith({
      output: expect.objectContaining({ limiter: expect.objectContaining({ enabled: false }) }),
    });
  });

  it("adjusts the limiter threshold", () => {
    const value = makeStudioValue();
    holder.value = value;
    render(<OutputPanel />);
    fireEvent.change(screen.getByLabelText("スレッショルド"), { target: { value: "0.6" } });
    expect(value.patchProject).toHaveBeenCalledWith({
      output: expect.objectContaining({ limiter: expect.objectContaining({ threshold: 0.6 }) }),
    });
  });
});
