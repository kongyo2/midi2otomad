import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { HelpPanel } from "./HelpPanel";

describe("HelpPanel", () => {
  it("renders the workflow guide", () => {
    render(<HelpPanel />);
    expect(screen.getByRole("heading", { name: "ワークフロー" })).toBeInTheDocument();
    expect(screen.getAllByRole("listitem")).toHaveLength(5);
    expect(screen.getByText(/高音質で書き出し/)).toBeInTheDocument();
  });
});
