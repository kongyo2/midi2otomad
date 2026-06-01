import { afterEach, describe, expect, it, vi } from "vitest";
import { makeId } from "./id";

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("makeId", () => {
  it("uses crypto.randomUUID when available", () => {
    vi.stubGlobal("crypto", { randomUUID: () => "abc-123" });
    expect(makeId("track")).toBe("track_abc-123");
  });

  it("defaults the prefix to 'id'", () => {
    vi.stubGlobal("crypto", { randomUUID: () => "abc-123" });
    expect(makeId()).toBe("id_abc-123");
  });

  it("falls back to time+random when crypto is undefined", () => {
    vi.stubGlobal("crypto", undefined);
    const id = makeId("sample");
    expect(id).toMatch(/^sample_[a-z0-9]+_[a-z0-9]{1,8}$/);
  });

  it("falls back when randomUUID is not a function", () => {
    vi.stubGlobal("crypto", {});
    const id = makeId();
    expect(id).toMatch(/^id_[a-z0-9]+_[a-z0-9]{1,8}$/);
  });

  it("produces unique ids across calls", () => {
    vi.stubGlobal("crypto", undefined);
    const ids = new Set(Array.from({ length: 50 }, () => makeId()));
    expect(ids.size).toBe(50);
  });
});
