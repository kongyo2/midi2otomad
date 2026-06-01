// @vitest-environment node
import { afterEach, describe, expect, it, vi } from "vitest";
import { IPC, type BridgeApi } from "../shared/ipc";
import type { BounceRequest } from "../shared/media";

const electron = vi.hoisted(() => ({
  exposeInMainWorld: vi.fn(),
  invoke: vi.fn(async () => "ok"),
}));

vi.mock("electron", () => ({
  contextBridge: { exposeInMainWorld: electron.exposeInMainWorld },
  ipcRenderer: { invoke: electron.invoke },
}));

afterEach(() => {
  vi.resetModules();
  vi.clearAllMocks();
});

describe("preload bridge", () => {
  it("exposes an api that forwards every call over IPC", async () => {
    await import("./index");
    expect(electron.exposeInMainWorld).toHaveBeenCalledWith("api", expect.any(Object));
    const api = electron.exposeInMainWorld.mock.calls[0]![1] as BridgeApi;

    await api.ping();
    await api.getVersion();
    await api.defaultProject();
    await api.probeMedia();
    await api.openMidi();
    await api.openAudio();
    const request = {
      format: "wav",
      pcm: { sampleRate: 48000, left: new Float32Array(1), right: new Float32Array(1), frames: 1 },
      defaultName: "song",
    } satisfies BounceRequest;
    await api.bounce(request);

    expect(electron.invoke.mock.calls).toEqual([
      [IPC.ping],
      [IPC.getVersion],
      [IPC.defaultProject],
      [IPC.probeMedia],
      [IPC.openMidi],
      [IPC.openAudio],
      [IPC.bounce, request],
    ]);
  });

  it("logs an error when the bridge cannot be exposed", async () => {
    const failure = new Error("contextBridge unavailable");
    electron.exposeInMainWorld.mockImplementationOnce(() => {
      throw failure;
    });
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => undefined);

    await import("./index");

    expect(consoleError).toHaveBeenCalledWith(failure);
    consoleError.mockRestore();
  });
});
