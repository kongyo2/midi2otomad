import { describe, expect, it } from "vitest";
import { formatBytes, formatDb, formatTime } from "./format";

describe("formatTime", () => {
  it("formats minutes, seconds and milliseconds", () => {
    expect(formatTime(65.123)).toBe("1:05.123");
  });

  it("pads seconds and milliseconds", () => {
    expect(formatTime(5.5)).toBe("0:05.500");
    expect(formatTime(9.125)).toBe("0:09.125");
    expect(formatTime(125.0625)).toBe("2:05.062");
  });

  it("clamps negative input to zero", () => {
    expect(formatTime(-3)).toBe("0:00.000");
  });

  it("treats non-finite input as zero", () => {
    expect(formatTime(Number.NaN)).toBe("0:00.000");
    expect(formatTime(Number.POSITIVE_INFINITY)).toBe("0:00.000");
  });
});

describe("formatBytes", () => {
  it("formats bytes below 1 KiB", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1023)).toBe("1023 B");
  });

  it("formats kibibytes with one decimal", () => {
    expect(formatBytes(1024)).toBe("1.0 KB");
    expect(formatBytes(1536)).toBe("1.5 KB");
  });

  it("formats mebibytes with one decimal", () => {
    expect(formatBytes(1024 * 1024)).toBe("1.0 MB");
    expect(formatBytes(5 * 1024 * 1024)).toBe("5.0 MB");
  });
});

describe("formatDb", () => {
  it("returns -∞ for silence", () => {
    expect(formatDb(0)).toBe("-∞");
    expect(formatDb(0.000001)).toBe("-∞");
  });

  it("shows a leading + at or above unity gain", () => {
    expect(formatDb(1)).toBe("+0.0 dB");
    expect(formatDb(2)).toBe("+6.0 dB");
  });

  it("shows attenuation without a + sign", () => {
    expect(formatDb(0.5)).toBe("-6.0 dB");
  });
});
