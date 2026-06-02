import { describe, expect, it } from "vitest";
import { allocateVoices, type VoiceRequest } from "./polyphony";
import type { Polyphony } from "../schemas/project";

function req(over: Partial<VoiceRequest> = {}): VoiceRequest {
  return { pitch: 60, startSec: 0, durationSec: 1, sampleId: "s1", ...over };
}

function poly(over: Partial<Polyphony> = {}): Polyphony {
  return { maxVoices: 0, priority: "newest", stopMode: "none", ...over };
}

describe("allocateVoices pass-through", () => {
  it("returns every note untouched when polyphony is unlimited and ungrouped", () => {
    const requests = [req({ startSec: 0 }), req({ startSec: 1 }), req({ startSec: 2 })];
    expect(allocateVoices(requests, poly())).toEqual([
      { index: 0, durationSec: 1 },
      { index: 1, durationSec: 1 },
      { index: 2, durationSec: 1 },
    ]);
  });

  it("preserves the original index so callers can map back to their notes", () => {
    const result = allocateVoices([req({ pitch: 48 }), req({ pitch: 72 })], poly());
    expect(result.map((r) => r.index)).toEqual([0, 1]);
  });

  it("keeps surviving notes in input order regardless of start time", () => {
    const requests = [req({ startSec: 2 }), req({ startSec: 0 }), req({ startSec: 1 })];
    expect(allocateVoices(requests, poly()).map((r) => r.index)).toEqual([0, 1, 2]);
  });
});

describe("allocateVoices voice cap", () => {
  it("admits every overlapping voice when the cap is zero (unlimited)", () => {
    const requests = [
      req({ startSec: 0, durationSec: 5 }),
      req({ startSec: 1, durationSec: 5 }),
      req({ startSec: 2, durationSec: 5 }),
    ];
    expect(allocateVoices(requests, poly({ maxVoices: 0 }))).toEqual([
      { index: 0, durationSec: 5 },
      { index: 1, durationSec: 5 },
      { index: 2, durationSec: 5 },
    ]);
  });

  it("truncates the oldest held voice when a new note exceeds the cap", () => {
    const requests = [
      req({ startSec: 0, durationSec: 5, pitch: 60 }),
      req({ startSec: 1, durationSec: 5, pitch: 62 }),
      req({ startSec: 2, durationSec: 5, pitch: 64 }),
    ];
    expect(allocateVoices(requests, poly({ maxVoices: 2, priority: "newest" }))).toEqual([
      { index: 0, durationSec: 2 },
      { index: 1, durationSec: 5 },
      { index: 2, durationSec: 5 },
    ]);
  });

  it("counts a voice as freed once its gate has closed", () => {
    const requests = [req({ startSec: 0, durationSec: 1 }), req({ startSec: 2, durationSec: 1 })];
    expect(allocateVoices(requests, poly({ maxVoices: 1 }))).toEqual([
      { index: 0, durationSec: 1 },
      { index: 1, durationSec: 1 },
    ]);
  });
});

describe("allocateVoices stealing priority", () => {
  it("oldest priority drops the incoming note and keeps the held voices", () => {
    const requests = [
      req({ startSec: 0, durationSec: 5 }),
      req({ startSec: 1, durationSec: 5 }),
      req({ startSec: 2, durationSec: 5 }),
    ];
    expect(allocateVoices(requests, poly({ maxVoices: 2, priority: "oldest" }))).toEqual([
      { index: 0, durationSec: 5 },
      { index: 1, durationSec: 5 },
    ]);
  });

  it("highest priority steals the lowest-pitched held voice", () => {
    const requests = [
      req({ startSec: 0, durationSec: 5, pitch: 72 }),
      req({ startSec: 1, durationSec: 5, pitch: 60 }),
      req({ startSec: 2, durationSec: 5, pitch: 64 }),
    ];
    expect(allocateVoices(requests, poly({ maxVoices: 2, priority: "highest" }))).toEqual([
      { index: 0, durationSec: 5 },
      { index: 1, durationSec: 1 },
      { index: 2, durationSec: 5 },
    ]);
  });

  it("lowest priority steals the highest-pitched held voice", () => {
    const requests = [
      req({ startSec: 0, durationSec: 5, pitch: 60 }),
      req({ startSec: 1, durationSec: 5, pitch: 72 }),
      req({ startSec: 2, durationSec: 5, pitch: 64 }),
    ];
    expect(allocateVoices(requests, poly({ maxVoices: 2, priority: "lowest" }))).toEqual([
      { index: 0, durationSec: 5 },
      { index: 1, durationSec: 1 },
      { index: 2, durationSec: 5 },
    ]);
  });

  it("drops, rather than truncates, a simultaneous loser to avoid a zero-length voice", () => {
    const requests = [
      req({ startSec: 0, durationSec: 5, pitch: 60 }),
      req({ startSec: 0, durationSec: 5, pitch: 62 }),
      req({ startSec: 0, durationSec: 5, pitch: 64 }),
    ];
    expect(allocateVoices(requests, poly({ maxVoices: 2, priority: "newest" }))).toEqual([
      { index: 1, durationSec: 5 },
      { index: 2, durationSec: 5 },
    ]);
  });
});

describe("allocateVoices stop mode", () => {
  it("none stop mode never chokes overlapping notes", () => {
    const requests = [req({ startSec: 0, durationSec: 5, pitch: 60 }), req({ startSec: 2, durationSec: 5, pitch: 60 })];
    expect(allocateVoices(requests, poly({ stopMode: "none" }))).toEqual([
      { index: 0, durationSec: 5 },
      { index: 1, durationSec: 5 },
    ]);
  });

  it("pitch stop mode chokes an earlier voice of the same pitch", () => {
    const requests = [req({ startSec: 0, durationSec: 5, pitch: 60 }), req({ startSec: 2, durationSec: 5, pitch: 60 })];
    expect(allocateVoices(requests, poly({ stopMode: "pitch" }))).toEqual([
      { index: 0, durationSec: 2 },
      { index: 1, durationSec: 5 },
    ]);
  });

  it("pitch stop mode leaves a different pitch sounding", () => {
    const requests = [req({ startSec: 0, durationSec: 5, pitch: 60 }), req({ startSec: 2, durationSec: 5, pitch: 64 })];
    expect(allocateVoices(requests, poly({ stopMode: "pitch" }))).toEqual([
      { index: 0, durationSec: 5 },
      { index: 1, durationSec: 5 },
    ]);
  });

  it("sample stop mode chokes an earlier voice sharing the same sample id", () => {
    const requests = [
      req({ startSec: 0, durationSec: 5, pitch: 60, sampleId: "a" }),
      req({ startSec: 2, durationSec: 5, pitch: 64, sampleId: "a" }),
    ];
    expect(allocateVoices(requests, poly({ stopMode: "sample" }))).toEqual([
      { index: 0, durationSec: 2 },
      { index: 1, durationSec: 5 },
    ]);
  });

  it("sample stop mode leaves a different sample sounding", () => {
    const requests = [
      req({ startSec: 0, durationSec: 5, pitch: 60, sampleId: "a" }),
      req({ startSec: 2, durationSec: 5, pitch: 60, sampleId: "b" }),
    ];
    expect(allocateVoices(requests, poly({ stopMode: "sample" }))).toEqual([
      { index: 0, durationSec: 5 },
      { index: 1, durationSec: 5 },
    ]);
  });

  it("track stop mode chokes every earlier voice on the track", () => {
    const requests = [
      req({ startSec: 0, durationSec: 5, pitch: 60, sampleId: "a" }),
      req({ startSec: 1, durationSec: 5, pitch: 64, sampleId: "b" }),
      req({ startSec: 2, durationSec: 5, pitch: 67, sampleId: "c" }),
    ];
    expect(allocateVoices(requests, poly({ stopMode: "track" }))).toEqual([
      { index: 0, durationSec: 1 },
      { index: 1, durationSec: 1 },
      { index: 2, durationSec: 5 },
    ]);
  });

  it("does not choke a group sibling that starts at the same instant", () => {
    const requests = [req({ startSec: 0, durationSec: 5, pitch: 60 }), req({ startSec: 0, durationSec: 5, pitch: 60 })];
    expect(allocateVoices(requests, poly({ stopMode: "pitch" }))).toEqual([
      { index: 0, durationSec: 5 },
      { index: 1, durationSec: 5 },
    ]);
  });
});

describe("allocateVoices deterministic tie-breaks", () => {
  it("oldest priority keeps the earliest-queued of simultaneous notes", () => {
    const requests = [
      req({ startSec: 0, durationSec: 5, pitch: 60 }),
      req({ startSec: 0, durationSec: 5, pitch: 62 }),
      req({ startSec: 0, durationSec: 5, pitch: 64 }),
    ];
    expect(allocateVoices(requests, poly({ maxVoices: 2, priority: "oldest" }))).toEqual([
      { index: 0, durationSec: 5 },
      { index: 1, durationSec: 5 },
    ]);
  });

  it("highest priority breaks a pitch tie by stealing the earliest-queued voice", () => {
    const requests = [
      req({ startSec: 0, durationSec: 5, pitch: 60 }),
      req({ startSec: 1, durationSec: 5, pitch: 60 }),
      req({ startSec: 2, durationSec: 5, pitch: 60 }),
    ];
    expect(allocateVoices(requests, poly({ maxVoices: 2, priority: "highest" }))).toEqual([
      { index: 0, durationSec: 2 },
      { index: 1, durationSec: 5 },
      { index: 2, durationSec: 5 },
    ]);
  });

  it("lowest priority breaks a pitch tie by stealing the earliest-queued voice", () => {
    const requests = [
      req({ startSec: 0, durationSec: 5, pitch: 60 }),
      req({ startSec: 1, durationSec: 5, pitch: 60 }),
      req({ startSec: 2, durationSec: 5, pitch: 60 }),
    ];
    expect(allocateVoices(requests, poly({ maxVoices: 2, priority: "lowest" }))).toEqual([
      { index: 0, durationSec: 2 },
      { index: 1, durationSec: 5 },
      { index: 2, durationSec: 5 },
    ]);
  });
});
