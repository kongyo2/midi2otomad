// @vitest-environment node
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

const PNG_SIGNATURE = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);

function readPngSize(buf: Buffer): { width: number; height: number } {
  return { width: buf.readUInt32BE(16), height: buf.readUInt32BE(20) };
}

describe("application icons", () => {
  it("ships a 1024px PNG for Linux and electron-builder fallbacks", () => {
    const file = join(root, "build", "icon.png");
    expect(existsSync(file)).toBe(true);
    const buf = readFileSync(file);
    expect(buf.subarray(0, 8).equals(PNG_SIGNATURE)).toBe(true);
    expect(readPngSize(buf)).toEqual({ width: 1024, height: 1024 });
  });

  it("ships a 256px PNG for the runtime window icon", () => {
    const file = join(root, "resources", "icon.png");
    expect(existsSync(file)).toBe(true);
    const buf = readFileSync(file);
    expect(buf.subarray(0, 8).equals(PNG_SIGNATURE)).toBe(true);
    expect(readPngSize(buf)).toEqual({ width: 256, height: 256 });
  });

  it("ships a multi-resolution Windows .ico", () => {
    const buf = readFileSync(join(root, "build", "icon.ico"));
    expect(buf.readUInt16LE(0)).toBe(0);
    expect(buf.readUInt16LE(2)).toBe(1);
    const count = buf.readUInt16LE(4);
    expect(count).toBeGreaterThanOrEqual(6);

    for (let i = 0; i < count; i += 1) {
      const entry = 6 + i * 16;
      const length = buf.readUInt32LE(entry + 8);
      const offset = buf.readUInt32LE(entry + 12);
      expect(buf.subarray(offset, offset + 8).equals(PNG_SIGNATURE)).toBe(true);
      expect(offset + length).toBeLessThanOrEqual(buf.length);
    }
  });

  it("ships a structurally valid macOS .icns", () => {
    const buf = readFileSync(join(root, "build", "icon.icns"));
    expect(buf.toString("latin1", 0, 4)).toBe("icns");
    expect(buf.readUInt32BE(4)).toBe(buf.length);

    let cursor = 8;
    let entries = 0;
    while (cursor < buf.length) {
      const length = buf.readUInt32BE(cursor + 4);
      expect(length).toBeGreaterThan(8);
      cursor += length;
      entries += 1;
    }
    expect(cursor).toBe(buf.length);
    expect(entries).toBeGreaterThanOrEqual(7);
  });
});
