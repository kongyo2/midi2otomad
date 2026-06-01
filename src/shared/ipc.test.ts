import { describe, expect, it } from "vitest";
import { IPC } from "./ipc";

describe("IPC channel map", () => {
  it("exposes the expected channel names", () => {
    expect(IPC).toEqual({
      ping: "app:ping",
      getVersion: "app:getVersion",
      defaultProject: "app:defaultProject",
      probeMedia: "media:probe",
      openMidi: "dialog:openMidi",
      openAudio: "dialog:openAudio",
      bounce: "export:bounce",
    });
  });

  it("uses unique channel strings", () => {
    const channels = Object.values(IPC);
    expect(new Set(channels).size).toBe(channels.length);
  });
});
