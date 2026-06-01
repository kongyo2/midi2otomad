import { describe, expect, it, vi, beforeEach } from "vitest";
import type { BrowserWindow } from "electron";
import type { BounceRequest } from "../shared/media";

const mocks = vi.hoisted(() => ({
  showOpenDialog: vi.fn(),
  showSaveDialog: vi.fn(),
  readFile: vi.fn(async () => Buffer.from([1, 2, 3])),
  writeExport: vi.fn(async () => ({ path: "/chosen.wav", bytes: 2048, durationSec: 1.5 })),
}));

vi.mock("electron", () => ({ dialog: { showOpenDialog: mocks.showOpenDialog, showSaveDialog: mocks.showSaveDialog } }));
vi.mock("node:fs/promises", () => ({ readFile: mocks.readFile }));
vi.mock("./media/encode", () => ({ writeExport: mocks.writeExport }));

import { bounce, openAudio, openMidi } from "./dialogs";

const fakeWindow = {} as BrowserWindow;

function pcm(): BounceRequest["pcm"] {
  return { sampleRate: 48000, left: new Float32Array(4), right: new Float32Array(4), frames: 4 };
}

beforeEach(() => {
  vi.clearAllMocks();
  mocks.readFile.mockResolvedValue(Buffer.from([1, 2, 3]));
  mocks.writeExport.mockResolvedValue({ path: "/chosen.wav", bytes: 2048, durationSec: 1.5 });
});

describe("openMidi", () => {
  it("loads the chosen file without a parent window", async () => {
    mocks.showOpenDialog.mockResolvedValue({ canceled: false, filePaths: ["/songs/demo.mid"] });
    const result = await openMidi(null);
    expect(mocks.showOpenDialog).toHaveBeenCalledTimes(1);
    expect(mocks.showOpenDialog.mock.calls[0]!).toHaveLength(1);
    expect(result).toEqual({ name: "demo.mid", data: new Uint8Array([1, 2, 3]) });
  });

  it("passes the parent window through to the dialog", async () => {
    mocks.showOpenDialog.mockResolvedValue({ canceled: false, filePaths: ["/songs/demo.mid"] });
    await openMidi(fakeWindow);
    expect(mocks.showOpenDialog.mock.calls[0]!).toHaveLength(2);
    expect(mocks.showOpenDialog.mock.calls[0]![0]).toBe(fakeWindow);
  });

  it("returns null when canceled", async () => {
    mocks.showOpenDialog.mockResolvedValue({ canceled: true, filePaths: [] });
    expect(await openMidi(null)).toBeNull();
  });

  it("returns null when no file path is present", async () => {
    mocks.showOpenDialog.mockResolvedValue({ canceled: false, filePaths: [] });
    expect(await openMidi(null)).toBeNull();
  });
});

describe("openAudio", () => {
  it("loads every selected file", async () => {
    mocks.showOpenDialog.mockResolvedValue({ canceled: false, filePaths: ["/a.wav", "/b.mp3"] });
    const result = await openAudio(fakeWindow);
    expect(result).toEqual([
      { name: "a.wav", data: new Uint8Array([1, 2, 3]) },
      { name: "b.mp3", data: new Uint8Array([1, 2, 3]) },
    ]);
  });

  it("returns null when canceled", async () => {
    mocks.showOpenDialog.mockResolvedValue({ canceled: true, filePaths: [] });
    expect(await openAudio(null)).toBeNull();
  });

  it("returns null when nothing is selected", async () => {
    mocks.showOpenDialog.mockResolvedValue({ canceled: false, filePaths: [] });
    expect(await openAudio(null)).toBeNull();
  });
});

describe("bounce", () => {
  it("exports a wav, appending the extension and forwarding options", async () => {
    mocks.showSaveDialog.mockResolvedValue({ canceled: false, filePath: "/out/song.wav" });
    const result = await bounce(fakeWindow, {
      format: "wav",
      pcm: pcm(),
      defaultName: "song",
      wavBitDepth: 16,
      mp3Bitrate: 256,
    });
    expect(mocks.showSaveDialog.mock.calls[0]![1]!.defaultPath).toBe("song.wav");
    expect(mocks.writeExport).toHaveBeenCalledWith(expect.anything(), {
      format: "wav",
      path: "/out/song.wav",
      wavBitDepth: 16,
      mp3Bitrate: 256,
    });
    expect(result).toEqual({ ok: true, path: "/chosen.wav", bytes: 2048, durationSec: 1.5 });
  });

  it("keeps a default name that already has the extension and omits absent options", async () => {
    mocks.showSaveDialog.mockResolvedValue({ canceled: false, filePath: "/out/song.mp3" });
    await bounce(null, { format: "mp3", pcm: pcm(), defaultName: "song.mp3" });
    expect(mocks.showSaveDialog.mock.calls[0]!).toHaveLength(1);
    expect(mocks.writeExport).toHaveBeenCalledWith(expect.anything(), { format: "mp3", path: "/out/song.mp3" });
  });

  it("reports cancellation", async () => {
    mocks.showSaveDialog.mockResolvedValue({ canceled: true, filePath: undefined });
    expect(await bounce(null, { format: "wav", pcm: pcm(), defaultName: "x" })).toEqual({
      ok: false,
      canceled: true,
    });
  });

  it("reports a missing target path as cancellation", async () => {
    mocks.showSaveDialog.mockResolvedValue({ canceled: false, filePath: undefined });
    expect(await bounce(null, { format: "wav", pcm: pcm(), defaultName: "x" })).toEqual({
      ok: false,
      canceled: true,
    });
  });

  it("surfaces an Error message from the encoder", async () => {
    mocks.showSaveDialog.mockResolvedValue({ canceled: false, filePath: "/out/x.wav" });
    mocks.writeExport.mockRejectedValueOnce(new Error("disk full"));
    expect(await bounce(null, { format: "wav", pcm: pcm(), defaultName: "x" })).toEqual({
      ok: false,
      canceled: false,
      error: "disk full",
    });
  });

  it("stringifies non-Error failures", async () => {
    mocks.showSaveDialog.mockResolvedValue({ canceled: false, filePath: "/out/x.wav" });
    mocks.writeExport.mockRejectedValueOnce("boom");
    expect(await bounce(null, { format: "wav", pcm: pcm(), defaultName: "x" })).toEqual({
      ok: false,
      canceled: false,
      error: "boom",
    });
  });
});
