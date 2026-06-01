import type { Project } from "./schemas/project";
import type { MediaProbe } from "./media";

export interface BridgeApi {
  ping: () => Promise<string>;
  getVersion: () => Promise<string>;
  defaultProject: () => Promise<Project>;
  probeMedia: () => Promise<MediaProbe>;
}

export const IPC = {
  ping: "app:ping",
  getVersion: "app:getVersion",
  defaultProject: "app:defaultProject",
  probeMedia: "media:probe",
} as const;
