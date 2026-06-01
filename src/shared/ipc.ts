import type { Project } from "./schemas/project";
import type { BounceRequest, BounceResponse, LoadedFile, MediaProbe } from "./media";

export interface BridgeApi {
  ping: () => Promise<string>;
  getVersion: () => Promise<string>;
  defaultProject: () => Promise<Project>;
  probeMedia: () => Promise<MediaProbe>;
  openMidi: () => Promise<LoadedFile | null>;
  openAudio: () => Promise<LoadedFile[] | null>;
  bounce: (request: BounceRequest) => Promise<BounceResponse>;
}

export const IPC = {
  ping: "app:ping",
  getVersion: "app:getVersion",
  defaultProject: "app:defaultProject",
  probeMedia: "media:probe",
  openMidi: "dialog:openMidi",
  openAudio: "dialog:openAudio",
  bounce: "export:bounce",
} as const;
