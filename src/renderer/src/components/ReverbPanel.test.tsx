import { describe, expect, it, vi, beforeEach } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import type { StudioContextValue } from "../state/StudioContext";
import { makeStudioValue } from "../../../test/studio";

const holder = vi.hoisted(() => ({ value: null as StudioContextValue | null }));
vi.mock("../state/StudioContext", () => ({ useStudio: () => holder.value }));

import { ReverbPanel } from "./ReverbPanel";

beforeEach(() => {
  holder.value = makeStudioValue();
});

describe("ReverbPanel", () => {
  it("toggles the reverb bus", () => {
    const value = makeStudioValue();
    holder.value = value;
    render(<ReverbPanel />);
    fireEvent.click(screen.getByLabelText("リバーブ"));
    expect(value.patchProject).toHaveBeenCalledWith({
      reverb: expect.objectContaining({ enabled: true }),
    });
  });

  it("edits the reverb parameters", () => {
    const value = makeStudioValue();
    holder.value = value;
    render(<ReverbPanel />);
    fireEvent.change(screen.getByLabelText("ルームサイズ"), { target: { value: "0.9" } });
    expect(value.patchProject).toHaveBeenCalledWith({
      reverb: expect.objectContaining({ roomSize: 0.9 }),
    });
    fireEvent.change(screen.getByLabelText("ウェット量"), { target: { value: "0.6" } });
    expect(value.patchProject).toHaveBeenCalledWith({
      reverb: expect.objectContaining({ wet: 0.6 }),
    });
    fireEvent.change(screen.getByLabelText("プリディレイ"), { target: { value: "40" } });
    expect(value.patchProject).toHaveBeenCalledWith({
      reverb: expect.objectContaining({ preDelayMs: 40 }),
    });
  });
});
